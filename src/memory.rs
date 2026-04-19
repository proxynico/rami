use crate::model::{MemoryPressure, MemorySnapshot};
use libc::{
    c_void, host_statistics64, mach_msg_type_number_t, size_t, sysctlbyname, vm_page_size,
    vm_statistics64, HOST_VM_INFO64, HOST_VM_INFO64_COUNT,
};
use std::io;
use std::mem::size_of;
use std::ptr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCounts {
    pub total_bytes: u64,
    pub page_size: u64,
    pub active_pages: u64,
    pub wired_pages: u64,
    pub compressed_pages: u64,
}

pub fn snapshot_from_counts(counts: MemoryCounts) -> MemorySnapshot {
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
        pressure: MemoryPressure::Normal,
        swap_used_bytes: 0,
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

pub struct MemorySampler {
    total_bytes: u64,
    page_size: u64,
}

impl MemorySampler {
    pub fn new() -> io::Result<Self> {
        let total_bytes = total_memory_bytes()?;
        let page_size = page_size_bytes()?;
        Ok(Self {
            total_bytes,
            page_size,
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

        Ok(snapshot_from_counts(MemoryCounts {
            total_bytes: self.total_bytes,
            page_size: self.page_size,
            active_pages: stats.active_count as u64,
            wired_pages: stats.wire_count as u64,
            compressed_pages: stats.compressor_page_count as u64,
        }))
    }
}

fn total_memory_bytes() -> io::Result<u64> {
    let mut value: u64 = 0;
    let mut size = size_of::<u64>() as size_t;
    let name = b"hw.memsize\0";

    let rc = unsafe {
        sysctlbyname(
            name.as_ptr() as *const i8,
            &mut value as *mut _ as *mut c_void,
            &mut size,
            ptr::null_mut(),
            0,
        )
    };

    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(value)
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
