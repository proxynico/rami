use crate::model::GpuSnapshot;
use libc::{c_char, c_void};
use std::io;

type IoObjectId = libc::mach_port_t;
type CfTypeRef = *const c_void;
type CfStringRef = *const c_void;
type CfDictionaryRef = *const c_void;
type CfMutableDictionaryRef = *mut c_void;
type CfTypeId = usize;
type CfIndex = isize;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const CF_NUMBER_SINT64_TYPE: CfIndex = 4;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CfMutableDictionaryRef;
    fn IOServiceGetMatchingServices(
        main_port: libc::mach_port_t,
        matching: CfDictionaryRef,
        iterator: *mut IoObjectId,
    ) -> libc::kern_return_t;
    fn IORegistryEntryCreateCFProperty(
        entry: IoObjectId,
        key: CfStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> CfTypeRef;
    fn IOIteratorNext(iterator: IoObjectId) -> IoObjectId;
    fn IOObjectRelease(object: IoObjectId) -> libc::kern_return_t;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        string: *const c_char,
        encoding: u32,
    ) -> CfStringRef;
    fn CFGetTypeID(value: CfTypeRef) -> CfTypeId;
    fn CFDictionaryGetTypeID() -> CfTypeId;
    fn CFDictionaryGetValue(dictionary: CfDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFNumberGetTypeID() -> CfTypeId;
    fn CFNumberGetValue(number: CfTypeRef, number_type: CfIndex, value: *mut c_void) -> u8;
    fn CFRelease(value: CfTypeRef);
}

pub(crate) struct GpuSampler;

impl GpuSampler {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn sample(&self) -> io::Result<Option<GpuSnapshot>> {
        read_gpu_utilization().map(|utilization| {
            utilization.map(|utilization_percent| GpuSnapshot {
                utilization_percent,
            })
        })
    }
}

fn read_gpu_utilization() -> io::Result<Option<u8>> {
    let matching = unsafe { IOServiceMatching(c"IOAccelerator".as_ptr()) };
    if matching.is_null() {
        return Err(io::Error::other(
            "IOServiceMatching returned no IOAccelerator dictionary",
        ));
    }

    let mut iterator = 0;
    let result = unsafe { IOServiceGetMatchingServices(0, matching, &mut iterator) };
    if result != 0 {
        return Err(io::Error::other(format!(
            "IOServiceGetMatchingServices failed with kern_return_t {result}"
        )));
    }
    let Some(iterator) = IoObject::new(iterator) else {
        return Ok(None);
    };

    let mut utilization = None;
    loop {
        let Some(accelerator) = IoObject::new(unsafe { IOIteratorNext(iterator.id()) }) else {
            break;
        };
        if let Some(value) = accelerator_utilization(accelerator.id()) {
            utilization = Some(utilization.map_or(value, |current: u8| current.max(value)));
        }
    }
    Ok(utilization)
}

fn accelerator_utilization(entry: IoObjectId) -> Option<u8> {
    let statistics = registry_property(entry, c"PerformanceStatistics")?;
    if unsafe { CFGetTypeID(statistics.get()) } != unsafe { CFDictionaryGetTypeID() } {
        return None;
    }
    let key = CfObject::new(unsafe {
        CFStringCreateWithCString(
            std::ptr::null(),
            c"Device Utilization %".as_ptr(),
            CF_STRING_ENCODING_UTF8,
        )
    })?;
    let value = unsafe { CFDictionaryGetValue(statistics.get(), key.get()) };
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }

    let mut raw = 0_i64;
    let converted =
        unsafe { CFNumberGetValue(value, CF_NUMBER_SINT64_TYPE, (&mut raw as *mut i64).cast()) };
    (converted != 0)
        .then(|| normalized_utilization(raw))
        .flatten()
}

fn registry_property(entry: IoObjectId, key: &std::ffi::CStr) -> Option<CfObject> {
    let key = CfObject::new(unsafe {
        CFStringCreateWithCString(std::ptr::null(), key.as_ptr(), CF_STRING_ENCODING_UTF8)
    })?;
    CfObject::new(unsafe { IORegistryEntryCreateCFProperty(entry, key.get(), std::ptr::null(), 0) })
}

fn normalized_utilization(raw: i64) -> Option<u8> {
    (0..=100).contains(&raw).then_some(raw as u8)
}

struct IoObject(IoObjectId);

impl IoObject {
    fn new(id: IoObjectId) -> Option<Self> {
        (id != 0).then_some(Self(id))
    }

    fn id(&self) -> IoObjectId {
        self.0
    }
}

impl Drop for IoObject {
    fn drop(&mut self) {
        let _ = unsafe { IOObjectRelease(self.0) };
    }
}

struct CfObject(CfTypeRef);

impl CfObject {
    fn new(value: CfTypeRef) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn get(&self) -> CfTypeRef {
        self.0
    }
}

impl Drop for CfObject {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_utilization_rejects_values_outside_percentage_range() {
        assert_eq!(normalized_utilization(-1), None);
        assert_eq!(normalized_utilization(0), Some(0));
        assert_eq!(normalized_utilization(76), Some(76));
        assert_eq!(normalized_utilization(101), None);
    }

    #[test]
    #[ignore = "requires a live macOS IOAccelerator"]
    fn smoke_reads_live_gpu_utilization() {
        let snapshot = GpuSampler::new()
            .sample()
            .expect("IOAccelerator lookup should succeed")
            .expect("this Mac should expose Device Utilization %");

        assert!(snapshot.utilization_percent <= 100);
    }
}
