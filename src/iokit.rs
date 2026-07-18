use libc::{c_char, c_void};

pub(crate) type IoObjectId = libc::mach_port_t;
pub(crate) type CfTypeRef = *const c_void;
pub(crate) type CfStringRef = *const c_void;
pub(crate) type CfDictionaryRef = *const c_void;
pub(crate) type CfMutableDictionaryRef = *mut c_void;
pub(crate) type CfTypeId = usize;
pub(crate) type CfIndex = isize;

pub(crate) const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub(crate) fn IOServiceMatching(name: *const c_char) -> CfMutableDictionaryRef;
    pub(crate) fn IOServiceGetMatchingServices(
        main_port: libc::mach_port_t,
        matching: CfDictionaryRef,
        iterator: *mut IoObjectId,
    ) -> libc::kern_return_t;
    pub(crate) fn IORegistryEntryFromPath(
        main_port: libc::mach_port_t,
        path: *const c_char,
    ) -> IoObjectId;
    pub(crate) fn IORegistryEntryGetChildIterator(
        entry: IoObjectId,
        plane: *const c_char,
        iterator: *mut IoObjectId,
    ) -> libc::kern_return_t;
    pub(crate) fn IORegistryEntryCreateCFProperty(
        entry: IoObjectId,
        key: CfStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> CfTypeRef;
    pub(crate) fn IOIteratorNext(iterator: IoObjectId) -> IoObjectId;
    pub(crate) fn IOObjectRelease(object: IoObjectId) -> libc::kern_return_t;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub(crate) fn CFStringCreateWithCString(
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
    pub(crate) fn CFDataGetLength(data: CfTypeRef) -> CfIndex;
    pub(crate) fn CFDataGetBytePtr(data: CfTypeRef) -> *const u8;
    pub(crate) fn CFRelease(value: CfTypeRef);
}

pub(crate) struct IoObject(IoObjectId);

impl IoObject {
    pub(crate) fn new(id: IoObjectId) -> Option<Self> {
        (id != 0).then_some(Self(id))
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
    pub(crate) fn new(value: CfTypeRef) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
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
