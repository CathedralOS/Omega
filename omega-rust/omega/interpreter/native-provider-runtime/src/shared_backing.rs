/*
We map the same physical file-object pages into both the host and worker
processes (`SharedBacking`) rather than copying per-call snapshots.

Each process owns its virtual memory mapping (`Mapping`) independently. Parent
unmapping or unlinking cannot free physical pages still mapped by a live
provider process. Backing files are never truncated or recycled; `read` and
`write` copy raw bytes without forming a Rust slice or reference over shared
memory that could alias concurrent foreign C accesses.
*/

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(unix)]
mod unix {
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
}

#[cfg(windows)]
mod windows {
    use std::{
        fs::File,
        io,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr::NonNull,
    };
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS,
        MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
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
}

#[cfg(unix)]
use unix::Mapping;
#[cfg(windows)]
use windows::Mapping;

pub(crate) struct SharedBacking {
    mapping: Mapping,
    extent: usize,
    _file: File,
}

impl SharedBacking {
    pub(crate) fn create(path: &Path, extent: usize) -> io::Result<Self> {
        if extent == 0 || extent > isize::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native backing extent is empty or unaddressable",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        file.set_len(extent as u64)?;
        Self::map(file, extent)
    }

    pub(crate) fn attach(path: &Path, extent: usize) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        if extent == 0 || extent > isize::MAX as usize || file.metadata()?.len() != extent as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "native backing differs from the admitted exact extent",
            ));
        }
        Self::map(file, extent)
    }

    fn map(file: File, extent: usize) -> io::Result<Self> {
        let mapping = Mapping::new(&file, extent)?;
        Ok(Self {
            mapping,
            extent,
            _file: file,
        })
    }

    pub(crate) fn pointer(
        &self,
        offset: usize,
        extent: usize,
        alignment: usize,
    ) -> io::Result<*mut u8> {
        if !alignment.is_power_of_two()
            || offset
                .checked_add(extent)
                .is_none_or(|end| end > self.extent)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native backing range is outside its exact home",
            ));
        }
        let address = unsafe { self.mapping.address().as_ptr().add(offset) };
        if !(address as usize).is_multiple_of(alignment) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native backing range has incompatible alignment",
            ));
        }
        Ok(address)
    }

    pub(crate) fn read(&self, offset: usize, output: &mut [u8]) -> io::Result<()> {
        let source = self.pointer(offset, output.len(), 1)?;
        unsafe {
            std::ptr::copy_nonoverlapping(source, output.as_mut_ptr(), output.len());
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, offset: usize, input: &[u8]) -> io::Result<()> {
        let destination = self.pointer(offset, input.len(), 1)?;
        unsafe {
            std::ptr::copy_nonoverlapping(input.as_ptr(), destination, input.len());
        }
        Ok(())
    }
}
