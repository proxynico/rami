use crate::model::{MemoryPressure, MemorySnapshot};
use libc::{
    boolean_t, c_int, c_void, host_statistics64, mach_msg_type_number_t, size_t, sysctlbyname,
    vm_page_size, vm_statistics64, HOST_VM_INFO64, HOST_VM_INFO64_COUNT,
};
use std::cell::Cell;
use std::io;
use std::mem::{size_of, MaybeUninit};

pub const VM_PRESSURE_NORMAL: i32 = 1;
pub const VM_PRESSURE_WARN: i32 = 2;
pub const VM_PRESSURE_CRITICAL: i32 = 4;

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
    pub wired_pages: u64,
    pub compressed_pages: u64,
}

pub fn pressure_from_raw(raw: i32) -> MemoryPressure {
    if raw & VM_PRESSURE_CRITICAL != 0 {
        MemoryPressure::High
    } else if raw & VM_PRESSURE_WARN != 0 {
        MemoryPressure::Elevated
    } else {
        MemoryPressure::Normal
    }
}

pub fn snapshot_from_counts(
    counts: MemoryCounts,
    pressure: MemoryPressure,
    swap_used_bytes: u64,
) -> MemorySnapshot {
    let used_bytes =
        (counts.active_pages + counts.wired_pages + counts.compressed_pages) * counts.page_size;

    let raw_percent = if counts.total_bytes == 0 {
        0.0
    } else {
        used_bytes as f64 / counts.total_bytes as f64 * 100.0
    };

    let used_percent = raw_percent.round().clamp(0.0, 100.0) as u8;

    MemorySnapshot {
        used_bytes,
        total_bytes: counts.total_bytes,
        used_percent,
        pressure,
        swap_used_bytes,
    }
}

pub fn validate_stats_count(count: u32) -> io::Result<()> {
    if count < HOST_VM_INFO64_COUNT {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "insufficient host statistics count: expected at least {}, got {}",
                HOST_VM_INFO64_COUNT, count
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
    total_bytes: u64,
    page_size: u64,
    cached_swap_used_bytes: Cell<u64>,
    ticks_until_swap_refresh: Cell<u8>,
}

impl MemorySampler {
    const SWAP_REFRESH_INTERVAL_TICKS: u8 = 6;

    pub fn new() -> io::Result<Self> {
        let total_bytes = total_memory_bytes()?;
        let page_size = page_size_bytes()?;
        Ok(Self {
            total_bytes,
            page_size,
            cached_swap_used_bytes: Cell::new(0),
            ticks_until_swap_refresh: Cell::new(0),
        })
    }

    pub fn sample(&self) -> io::Result<MemorySnapshot> {
        let mut stats = unsafe { std::mem::zeroed::<vm_statistics64>() };
        let mut count = HOST_VM_INFO64_COUNT;

        #[allow(deprecated)]
        let result = unsafe {
            host_statistics64(
                libc::mach_host_self(),
                HOST_VM_INFO64,
                &mut stats as *mut _ as *mut i32,
                &mut count as *mut mach_msg_type_number_t,
            )
        };

        if result != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("host_statistics64 failed with kern_return_t {}", result),
            ));
        }

        validate_stats_count(count)?;
        let pressure = read_pressure_level()?;
        let swap_used_bytes = self.swap_used_bytes()?;

        Ok(snapshot_from_counts(
            MemoryCounts {
                total_bytes: self.total_bytes,
                page_size: self.page_size,
                active_pages: stats.active_count as u64,
                wired_pages: stats.wire_count as u64,
                compressed_pages: stats.compressor_page_count as u64,
            },
            pressure,
            swap_used_bytes,
        ))
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

fn read_pressure_level() -> io::Result<MemoryPressure> {
    let raw: c_int = read_sysctl_value(b"kern.memorystatus_vm_pressure_level\0")?;
    Ok(pressure_from_raw(raw))
}

fn read_swap_used_bytes() -> io::Result<u64> {
    let usage: XswUsage = read_sysctl_value(b"vm.swapusage\0")?;
    Ok(usage.xsu_used)
}

fn total_memory_bytes() -> io::Result<u64> {
    read_sysctl_value(b"hw.memsize\0")
}

fn page_size_bytes() -> io::Result<u64> {
    let page_size = unsafe { vm_page_size as u64 };

    if page_size == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "vm_page_size unavailable",
        ));
    }

    Ok(page_size)
}

#[cfg(test)]
mod tests {
    use super::validate_sysctl_size;

    #[test]
    fn validate_sysctl_size_rejects_mismatched_byte_count() {
        let error = validate_sysctl_size(4, 8, "vm.swapusage")
            .expect_err("size mismatch should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("vm.swapusage"));
        assert!(error.to_string().contains("expected 8 bytes"));
        assert!(error.to_string().contains("got 4"));
    }
}
