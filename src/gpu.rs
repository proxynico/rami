use crate::iokit::{
    CFDictionaryGetTypeID, CFDictionaryGetValue, CFGetTypeID, CFNumberGetTypeID, CFNumberGetValue,
    CFStringCreateWithCString, CfIndex, CfObject, IOIteratorNext, IORegistryEntryCreateCFProperty,
    IOServiceGetMatchingServices, IOServiceMatching, IoObject, IoObjectId, CF_STRING_ENCODING_UTF8,
};
use crate::model::GpuSnapshot;
use std::io;

const CF_NUMBER_SINT64_TYPE: CfIndex = 4;

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
