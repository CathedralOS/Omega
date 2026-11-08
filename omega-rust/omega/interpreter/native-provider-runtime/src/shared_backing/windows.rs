use std::{
    fs::File,
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::NonNull,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    PAGE_READWRITE, UnmapViewOfFile,
};

pub(super) struct Mapping {
    address: NonNull<u8>,
    _object: OwnedHandle,
}
impl Mapping {
    pub(super) fn new(file: &File, extent: usize) -> io::Result<Self> {
        let handle = unsafe {
            CreateFileMappingW(
                file.as_raw_handle(),
                std::ptr::null(),
                PAGE_READWRITE,
                0,
                0,
                std::ptr::null(),
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let object = unsafe { OwnedHandle::from_raw_handle(handle) };
        let view = unsafe {
            MapViewOfFile(
                object.as_raw_handle(),
                FILE_MAP_READ | FILE_MAP_WRITE,
                0,
                0,
                extent,
            )
        };
        let address = NonNull::new(view.Value.cast()).ok_or_else(io::Error::last_os_error)?;
        Ok(Self {
            address,
            _object: object,
        })
    }
    pub(super) fn address(&self) -> NonNull<u8> {
        self.address
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.address.as_ptr().cast(),
            });
        }
    }
}
