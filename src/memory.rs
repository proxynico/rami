use crate::model::{MemorySnapshot, PressureSource, CRITICAL_PRESSURE_PCT, WARNING_PRESSURE_PCT};
use libc::{
    boolean_t, c_void, host_statistics64, mach_msg_type_number_t, size_t, sysctlbyname,
    vm_page_size, vm_statistics64, HOST_VM_INFO64, HOST_VM_INFO64_COUNT,
};
use std::cell::Cell;
use std::io;
use std::mem::{size_of, MaybeUninit};
use std::sync::OnceLock;

const FORCE_PRESSURE_ENV: &str = "RAMI_FORCE_PRESSURE";

/// Minimum `host_statistics64` word count covering every field
/// [`MemorySampler`] reads (`internal_page_count` is the last).
///
/// libc 0.2.187+ grew `vm_statistics64` (and thus `HOST_VM_INFO64_COUNT`) with
/// tagged-page fields the running kernel does not report. Darwin 25 answers 62
/// words against the SDK's 104; requiring the full SDK count rejected every
/// sample and left the dropdown on the placeholder forever.
const REQUIRED_STATS_COUNT: mach_msg_type_number_t =
    (std::mem::offset_of!(vm_statistics64, total_uncompressed_pages_in_compressor)
        / size_of::<libc::integer_t>()) as mach_msg_type_number_t;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct XswUsage {
    xsu_total: u64,
    xsu_avail: u64,
    xsu_used: u64,
    xsu_pagesize: u32,
    xsu_encrypted: boolean_t,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCounts {
    pub total_bytes: u64,
    pub page_size: u64,
    pub active_pages: u64,
    pub internal_pages: u64,
    pub wired_pages: u64,
    pub compressed_pages: u64,
    pub free_pages: u64,
    pub inactive_pages: u64,
    pub speculative_pages: u64,
    pub purgeable_pages: u64,
}

pub fn snapshot_from_counts(
    counts: MemoryCounts,
    swap_used_bytes: u64,
    kernel_available_percent: Option<i32>,
) -> MemorySnapshot {
    // "Used" = active + wired + compressed pages from host_statistics64. This is a simple,
    // stable definition; Activity Monitor's "Memory Used" applies extra app-memory
    // attribution, so this figure can drift from it by a few percent.
    let used_pages = counts
        .active_pages
        .saturating_add(counts.wired_pages)
        .saturating_add(counts.compressed_pages);
    let used_bytes = used_pages.saturating_mul(counts.page_size);

    // "Available" = free + inactive + purgeable: pages the system can reclaim without
    // swapping. A rough mirror of Activity Monitor's "Memory Available"; same
    // few-percent drift caveat as `used_bytes`.
    //
    // Speculative pages are NOT added here. `host_statistics64` reports
    // `free_count` as raw free pages *plus* speculative pages, so adding
    // `speculative_count` again double-counts them. Verified against the kernel on
    // Darwin 25: free_count 7368 - speculative_count 2838 == vm.page_free_count 4530,
    // exactly. This is also why `vm_stat` prints a smaller "Pages free" than
    // `free_count`.
    let available_pages = counts
        .free_pages
        .saturating_add(counts.inactive_pages)
        .saturating_add(counts.purgeable_pages);
    let available_bytes = available_pages.saturating_mul(counts.page_size);

    // Activity Monitor's App Memory is based on anonymous/internal pages. Purgeable
    // internal pages can be reclaimed, so do not attribute them to applications.
    let app_memory_bytes = counts
        .internal_pages
        .saturating_sub(counts.purgeable_pages)
        .saturating_mul(counts.page_size);
    let wired_bytes = counts.wired_pages.saturating_mul(counts.page_size);
    let compressed_bytes = counts.compressed_pages.saturating_mul(counts.page_size);
    // The Free row shows genuinely free pages, so strip the speculative pages that
    // `free_count` bundles in. Without this the row reads high against Activity
    // Monitor and `vm_stat` by the speculative count, which on a warm system is
    // routinely larger than free itself.
    let free_bytes = counts
        .free_pages
        .saturating_sub(counts.speculative_pages)
        .saturating_mul(counts.page_size);

    let raw_percent = if counts.total_bytes == 0 {
        0.0
    } else {
        used_bytes as f64 / counts.total_bytes as f64 * 100.0
    };

    let used_percent = raw_percent.round().clamp(0.0, 100.0) as u8;
    let (pressure_percent, pressure_source) = match kernel_available_percent {
        Some(available_percent) => (
            100_u8.saturating_sub(available_percent.clamp(0, 100) as u8),
            PressureSource::Kernel,
        ),
        None => {
            let available_percent = if counts.total_bytes == 0 {
                100.0
            } else {
                available_bytes as f64 / counts.total_bytes as f64 * 100.0
            };
            (
                100_u8.saturating_sub(available_percent.round().clamp(0.0, 100.0) as u8),
                PressureSource::AvailableFallback,
            )
        }
    };

    MemorySnapshot {
        used_bytes,
        total_bytes: counts.total_bytes,
        used_percent,
        pressure_percent,
        pressure_source,
        app_memory_bytes,
        wired_bytes,
        compressed_bytes,
        free_bytes,
        swap_used_bytes,
        available_bytes,
    }
}

/// Parse the `RAMI_FORCE_PRESSURE` development override into a pressure percent.
///
/// The Warning and Critical accents are otherwise unreachable without genuinely
/// exhausting system memory, so they went unverified — a UI pass could only ever
/// capture Normal. This maps a state name onto a percent that
/// [`classify_pressure`](crate::model::classify_pressure) will classify into that
/// band.
///
/// `warning` and `critical` deliberately resolve to the threshold values
/// themselves rather than the middle of each band, so previewing an accent also
/// exercises the `>=` boundary.
pub fn parse_forced_pressure(raw: &str) -> Option<u8> {
    match raw.trim().to_ascii_lowercase().as_str() {
        // Comfortably inside the Normal band, not adjacent to the warning edge.
        "normal" => Some(50),
        "warning" => Some(WARNING_PRESSURE_PCT),
        "critical" => Some(CRITICAL_PRESSURE_PCT),
        _ => None,
    }
}

/// The override, read from the environment exactly once.
///
/// Development hook only. It is compiled into release builds on purpose:
/// `scripts/build-app.sh` produces a release bundle, so gating it behind
/// `debug_assertions` would make it useless for checking the accents in the app
/// people actually run. It changes displayed pressure and nothing else.
fn forced_pressure() -> Option<u8> {
    static FORCED: OnceLock<Option<u8>> = OnceLock::new();
    *FORCED.get_or_init(|| {
        let raw = match std::env::var(FORCE_PRESSURE_ENV) {
            Ok(raw) => raw,
            // Unset is the normal case and stays silent.
            Err(std::env::VarError::NotPresent) => return None,
            // A value that is set but unreadable must not look like an unset one:
            // someone deliberately asked for an override and would otherwise get
            // no override and no explanation.
            Err(std::env::VarError::NotUnicode(_)) => {
                eprintln!("{FORCE_PRESSURE_ENV}: ignoring, value is not valid UTF-8");
                return None;
            }
        };
        match parse_forced_pressure(&raw) {
            Some(percent) => {
                eprintln!(
                    "{FORCE_PRESSURE_ENV}={raw}: forcing pressure to {percent}% (development override)"
                );
                Some(percent)
            }
            None => {
                eprintln!(
                    "{FORCE_PRESSURE_ENV}={raw}: ignoring, expected one of normal, warning, critical"
                );
                None
            }
        }
    })
}

pub fn validate_stats_count(count: u32) -> io::Result<()> {
    if count < REQUIRED_STATS_COUNT {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "insufficient host statistics count: expected at least {}, got {}",
                REQUIRED_STATS_COUNT, count
            ),
        ));
    }

    Ok(())
}

fn validate_sysctl_size(actual: size_t, expected: usize, name: &str) -> io::Result<()> {
    if actual != expected as size_t {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "sysctl {} returned unexpected size: expected {} bytes, got {}",
                name, expected, actual
            ),
        ));
    }

    Ok(())
}

pub struct MemorySampler {
    host_port: libc::mach_port_t,
    total_bytes: u64,
    page_size: u64,
    cached_swap_used_bytes: Cell<u64>,
    ticks_until_swap_refresh: Cell<u8>,
}

impl MemorySampler {
    const SWAP_REFRESH_INTERVAL_TICKS: u8 = 6;

    pub fn new() -> io::Result<Self> {
        #[allow(deprecated)]
        let host_port = unsafe { libc::mach_host_self() };
        let total_bytes = total_memory_bytes()?;
        let page_size = page_size_bytes()?;
        Ok(Self {
            host_port,
            total_bytes,
            page_size,
            cached_swap_used_bytes: Cell::new(0),
            ticks_until_swap_refresh: Cell::new(0),
        })
    }

    pub fn sample(&self) -> io::Result<MemorySnapshot> {
        let mut stats = unsafe { std::mem::zeroed::<vm_statistics64>() };
        let mut count = HOST_VM_INFO64_COUNT;

        let result = unsafe {
            host_statistics64(
                self.host_port,
                HOST_VM_INFO64,
                &mut stats as *mut _ as *mut i32,
                &mut count as *mut mach_msg_type_number_t,
            )
        };

        if result != 0 {
            return Err(io::Error::other(format!(
                "host_statistics64 failed with kern_return_t {}",
                result
            )));
        }

        validate_stats_count(count)?;
        let swap_used_bytes = self.swap_used_bytes()?;

        let kernel_available_percent = read_memory_status_level().ok();

        let snapshot = snapshot_from_counts(
            MemoryCounts {
                total_bytes: self.total_bytes,
                page_size: self.page_size,
                active_pages: stats.active_count as u64,
                internal_pages: stats.internal_page_count as u64,
                wired_pages: stats.wire_count as u64,
                compressed_pages: stats.compressor_page_count as u64,
                free_pages: stats.free_count as u64,
                inactive_pages: stats.inactive_count as u64,
                speculative_pages: stats.speculative_count as u64,
                purgeable_pages: stats.purgeable_count as u64,
            },
            swap_used_bytes,
            kernel_available_percent,
        );

        // Applied here rather than inside snapshot_from_counts so that function
        // stays a pure mapping from kernel counts and remains testable without
        // touching the environment.
        Ok(match forced_pressure() {
            Some(pressure_percent) => MemorySnapshot {
                pressure_percent,
                ..snapshot
            },
            None => snapshot,
        })
    }

    fn swap_used_bytes(&self) -> io::Result<u64> {
        if self.ticks_until_swap_refresh.get() == 0 {
            let swap_used_bytes = read_swap_used_bytes()?;
            self.cached_swap_used_bytes.set(swap_used_bytes);
            self.ticks_until_swap_refresh
                .set(Self::SWAP_REFRESH_INTERVAL_TICKS.saturating_sub(1));
            return Ok(swap_used_bytes);
        }

        self.ticks_until_swap_refresh
            .set(self.ticks_until_swap_refresh.get() - 1);
        Ok(self.cached_swap_used_bytes.get())
    }
}

fn read_sysctl_value<T: Copy>(name: &[u8]) -> io::Result<T> {
    let mut value = MaybeUninit::<T>::uninit();
    let expected_size = size_of::<T>();
    let mut size = expected_size as size_t;

    let rc = unsafe {
        sysctlbyname(
            name.as_ptr() as *const i8,
            value.as_mut_ptr() as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    let name = std::str::from_utf8(name)
        .ok()
        .and_then(|name| name.strip_suffix('\0'))
        .unwrap_or("<sysctl>");
    validate_sysctl_size(size, expected_size, name)?;

    Ok(unsafe { value.assume_init() })
}

fn read_swap_used_bytes() -> io::Result<u64> {
    let usage: XswUsage = read_sysctl_value(b"vm.swapusage\0")?;
    Ok(usage.xsu_used)
}

fn read_memory_status_level() -> io::Result<i32> {
    read_sysctl_value(b"kern.memorystatus_level\0")
}

fn total_memory_bytes() -> io::Result<u64> {
    read_sysctl_value(b"hw.memsize\0")
}

fn page_size_bytes() -> io::Result<u64> {
    let page_size = unsafe { vm_page_size as u64 };

    if page_size == 0 {
        return Err(io::Error::other("vm_page_size unavailable"));
    }

    Ok(page_size)
}

#[cfg(test)]
mod tests {
    use super::{validate_stats_count, validate_sysctl_size, REQUIRED_STATS_COUNT};
    use libc::HOST_VM_INFO64_COUNT;

    #[test]
    fn validate_sysctl_size_rejects_mismatched_byte_count() {
        let error = validate_sysctl_size(4, 8, "vm.swapusage")
            .expect_err("size mismatch should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("vm.swapusage"));
        assert!(error.to_string().contains("expected 8 bytes"));
        assert!(error.to_string().contains("got 4"));
    }

    #[test]
    fn stats_count_accepts_the_classic_kernel_reply() {
        // libc 0.2.187+ extended vm_statistics64 with tagged-page fields the
        // running kernel does not report: Darwin 25 answers 62 words against
        // the SDK's HOST_VM_INFO64_COUNT of 104. Requiring the full SDK count
        // rejected every sample and left the dropdown on the placeholder.
        assert!(validate_stats_count(62).is_ok());
        assert!(validate_stats_count(HOST_VM_INFO64_COUNT).is_ok());
    }

    #[test]
    fn stats_count_still_rejects_replies_missing_fields_rami_reads() {
        assert!(REQUIRED_STATS_COUNT <= 62, "must fit the classic struct");
        assert!(validate_stats_count(REQUIRED_STATS_COUNT - 1).is_err());
        assert!(validate_stats_count(REQUIRED_STATS_COUNT).is_ok());
    }
}
