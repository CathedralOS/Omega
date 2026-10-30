use std::{fs::File, io, os::fd::AsRawFd, ptr::NonNull};

pub(super) struct Mapping {
    address: NonNull<u8>,
    extent: usize,
}
impl Mapping {
    pub(super) fn new(file: &File, extent: usize) -> io::Result<Self> {
        let raw = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                extent,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        if raw == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        let Some(address) = NonNull::new(raw.cast()) else {
            unsafe {
                libc::munmap(raw, extent);
            }
            return Err(io::Error::other(
                "native backing mapped at the null address",
            ));
        };
        Ok(Self { address, extent })
    }
    pub(super) fn address(&self) -> NonNull<u8> {
        self.address
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.address.as_ptr().cast(), self.extent);
        }
    }
}
