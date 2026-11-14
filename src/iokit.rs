use libc::{c_char, c_void};
use std::ffi::CStr;

pub(crate) type IoObjectId = libc::mach_port_t;
pub(crate) type CfTypeRef = *const c_void;
pub(crate) type CfStringRef = *const c_void;
pub(crate) type CfDictionaryRef = *const c_void;
pub(crate) type CfMutableDictionaryRef = *mut c_void;
pub(crate) type CfTypeId = usize;
pub(crate) type CfIndex = isize;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub(crate) fn IOServiceMatching(name: *const c_char) -> CfMutableDictionaryRef;
    pub(crate) fn IOServiceGetMatchingServices(
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
    pub(crate) fn IOIteratorNext(iterator: IoObjectId) -> IoObjectId;
    pub(crate) fn IORegistryEntryFromPath(
        main_port: libc::mach_port_t,
        path: *const c_char,
    ) -> IoObjectId;
    pub(crate) fn IOObjectRelease(object: IoObjectId) -> libc::kern_return_t;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        string: *const c_char,
        encoding: u32,
    ) -> CfStringRef;
    pub(crate) fn CFGetTypeID(value: CfTypeRef) -> CfTypeId;
    pub(crate) fn CFDictionaryGetTypeID() -> CfTypeId;
    pub(crate) fn CFDictionaryGetValue(
        dictionary: CfDictionaryRef,
        key: *const c_void,
    ) -> *const c_void;
    pub(crate) fn CFNumberGetTypeID() -> CfTypeId;
    pub(crate) fn CFNumberGetValue(
        number: CfTypeRef,
        number_type: CfIndex,
        value: *mut c_void,
    ) -> u8;
    pub(crate) fn CFDataGetTypeID() -> CfTypeId;
    pub(crate) fn CFDataGetBytePtr(data: CfTypeRef) -> *const u8;
    pub(crate) fn CFDataGetLength(data: CfTypeRef) -> CfIndex;
    pub(crate) fn CFRelease(value: CfTypeRef);
}

pub(crate) struct IoObject(IoObjectId);

impl IoObject {
    // Not `then_some`: it builds the wrapper eagerly, and dropping the
    // unused one would release a null handle.
    pub(crate) fn new(id: IoObjectId) -> Option<Self> {
        if id == 0 {
            None
        } else {
            Some(Self(id))
        }
    }

    pub(crate) fn id(&self) -> IoObjectId {
        self.0
    }
}

impl Drop for IoObject {
    fn drop(&mut self) {
        let _ = unsafe { IOObjectRelease(self.0) };
    }
}

pub(crate) struct CfObject(CfTypeRef);

impl CfObject {
    // Not `then_some`, for the same reason: CFRelease(NULL) traps.
    pub(crate) fn new(value: CfTypeRef) -> Option<Self> {
        if value.is_null() {
            None
        } else {
            Some(Self(value))
        }
    }

    pub(crate) fn get(&self) -> CfTypeRef {
        self.0
    }
}

impl Drop for CfObject {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

pub(crate) fn cf_string(value: &CStr) -> Option<CfObject> {
    CfObject::new(unsafe {
        CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), CF_STRING_ENCODING_UTF8)
    })
}

pub(crate) fn registry_property(entry: IoObjectId, key: &CStr) -> Option<CfObject> {
    let key = cf_string(key)?;
    CfObject::new(unsafe { IORegistryEntryCreateCFProperty(entry, key.get(), std::ptr::null(), 0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_handles_build_no_wrapper() {
        assert!(CfObject::new(std::ptr::null()).is_none());
        assert!(IoObject::new(0).is_none());
    }
}
