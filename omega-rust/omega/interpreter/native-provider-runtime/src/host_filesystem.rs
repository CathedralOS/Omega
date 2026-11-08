/*
We execute Linux kernel filesystem operations over an instance-private guest
descriptor table (`LinuxFilesystem`).

The receiving runtime separately admits target, exact ABI, and delegation. Our
`libc::syscall` trampoline enters the selected kernel operation and translates
its immediate thread-local `errno` back into Linux's negative-status convention.
No Rust I/O error, retry loop, private worker descriptor, or modeled standard
stream (`0..=2`) supplies a kernel result. When `close` fails after retiring the
OS descriptor, we record a `None` tombstone so neither retries nor `OwnedFd`
destructors can close a subsequently reused OS descriptor number.
*/

use std::{ffi::CStr, io};

#[derive(Default)]
pub struct LinuxFilesystem {
    #[cfg(target_os = "linux")]
    descriptors: std::collections::BTreeMap<i32, Option<std::os::fd::OwnedFd>>,
}

impl LinuxFilesystem {
    pub fn open_at(
        &mut self,
        directory: i32,
        path: &CStr,
        flags: i32,
        mode: i32,
    ) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::FromRawFd;
            let directory = if directory == libc::AT_FDCWD {
                directory
            } else {
                self.descriptor(directory)?
            };
            let status = kernel_status(unsafe {
                libc::syscall(libc::SYS_openat, directory, path.as_ptr(), flags, mode)
            })?;
            if status < 0 {
                return Ok(status);
            }
            let native = i32::try_from(status)
                .map_err(|_| io::Error::other("kernel descriptor exceeds its native carrier"))?;
            let native = unsafe { std::os::fd::OwnedFd::from_raw_fd(native) };
            let guest = (3..=i32::MAX)
                .find(|guest| !self.descriptors.contains_key(guest))
                .ok_or_else(|| {
                    io::Error::other("guest filesystem descriptor namespace exhausted")
                })?;
            self.descriptors.insert(guest, Some(native));
            Ok(i64::from(guest))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (directory, path, flags, mode);
            Err(unsupported())
        }
    }

    pub fn lseek(&self, descriptor: i32, offset: i64, whence: i32) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let descriptor = self.descriptor(descriptor)?;
            kernel_status(unsafe { libc::syscall(libc::SYS_lseek, descriptor, offset, whence) })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (descriptor, offset, whence);
            Err(unsupported())
        }
    }

    pub fn unlinkat(&mut self, directory: i32, path: &CStr, flags: i32) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let directory = if directory == libc::AT_FDCWD {
                directory
            } else {
                self.descriptor(directory)?
            };
            kernel_status(unsafe {
                libc::syscall(libc::SYS_unlinkat, directory, path.as_ptr(), flags)
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (directory, path, flags);
            Err(unsupported())
        }
    }

    pub fn mkdirat(&self, directory: i32, path: &CStr, mode: i32) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let directory = if directory == libc::AT_FDCWD {
                directory
            } else {
                self.descriptor(directory)?
            };
            kernel_status(unsafe {
                libc::syscall(libc::SYS_mkdirat, directory, path.as_ptr(), mode)
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (directory, path, mode);
            Err(unsupported())
        }
    }

    pub fn stat_at(
        &self,
        directory: i32,
        path: &CStr,
        bytes: &mut [u8; 144],
        flags: i32,
    ) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let directory = if directory == libc::AT_FDCWD {
                directory
            } else {
                self.descriptor(directory)?
            };
            kernel_status(unsafe {
                libc::syscall(
                    libc::SYS_newfstatat,
                    directory,
                    path.as_ptr(),
                    bytes.as_mut_ptr(),
                    flags,
                )
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (directory, path, bytes, flags);
            Err(unsupported())
        }
    }

    pub fn stat(&self, descriptor: i32, bytes: &mut [u8; 144]) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let descriptor = self.descriptor(descriptor)?;
            kernel_status(unsafe { libc::syscall(libc::SYS_fstat, descriptor, bytes.as_mut_ptr()) })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (descriptor, bytes);
            Err(unsupported())
        }
    }

    pub fn directory_entries(&self, descriptor: i32, bytes: &mut [u8]) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let descriptor = self.descriptor(descriptor)?;
            kernel_status(unsafe {
                libc::syscall(
                    libc::SYS_getdents64,
                    descriptor,
                    bytes.as_mut_ptr(),
                    bytes.len(),
                )
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (descriptor, bytes);
            Err(unsupported())
        }
    }

    pub fn read(&self, descriptor: i32, bytes: &mut [u8]) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let descriptor = self.descriptor(descriptor)?;
            kernel_status(unsafe {
                libc::syscall(libc::SYS_read, descriptor, bytes.as_mut_ptr(), bytes.len())
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (descriptor, bytes);
            Err(unsupported())
        }
    }

    pub fn write(&self, descriptor: i32, bytes: &[u8]) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            let descriptor = self.descriptor(descriptor)?;
            kernel_status(unsafe {
                libc::syscall(libc::SYS_write, descriptor, bytes.as_ptr(), bytes.len())
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (descriptor, bytes);
            Err(unsupported())
        }
    }

    pub fn close(&mut self, descriptor: i32) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::IntoRawFd;
            self.descriptor(descriptor)?;
            let native = match self.descriptors.remove(&descriptor) {
                Some(Some(native)) => native.into_raw_fd(),
                None => -1,
                Some(None) => unreachable!("descriptor rejected its tombstone"),
            };
            let status = kernel_status(unsafe { libc::syscall(libc::SYS_close, native) });
            if !matches!(&status, Ok(0)) {
                self.descriptors.insert(descriptor, None);
            }
            status
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = descriptor;
            Err(unsupported())
        }
    }

    #[cfg(target_os = "linux")]
    fn descriptor(&self, guest: i32) -> io::Result<i32> {
        use std::os::fd::AsRawFd;
        if (0..=2).contains(&guest) {
            return Err(io::Error::other(
                "filesystem delegation does not supply host standard streams",
            ));
        }
        match self.descriptors.get(&guest) {
            Some(Some(native)) => Ok(native.as_raw_fd()),
            Some(None) => Err(io::Error::other(
                "filesystem descriptor is uncertain after close",
            )),
            None => Ok(-1),
        }
    }
}

#[cfg(target_os = "linux")]
fn kernel_status(status: libc::c_long) -> io::Result<i64> {
    if status != -1 {
        return Ok(status);
    }
    match io::Error::last_os_error().raw_os_error() {
        Some(errno @ 1..=4095) => Ok(-i64::from(errno)),
        _ => Err(io::Error::other(
            "kernel trampoline returned no Linux errno",
        )),
    }
}

#[cfg(not(target_os = "linux"))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Linux filesystem supply requires a Linux host",
    )
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::LinuxFilesystem;
    use std::ffi::CString;

    #[test]
    fn directory_lifecycle_uses_private_descriptors() -> std::io::Result<()> {
        let directory = std::env::temp_dir().join(format!(
            "omega-directory-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)?
                .as_nanos()
        ));
        let path = CString::new(directory.as_os_str().as_encoded_bytes())?;
        let mut filesystem = LinuxFilesystem::default();
        assert_eq!(filesystem.mkdirat(-100, &path, 0o700)?, 0);
        assert_eq!(
            filesystem.mkdirat(-100, &path, 0o700)?,
            -i64::from(libc::EEXIST)
        );
        let descriptor = i32::try_from(filesystem.open_at(-100, &path, libc::O_DIRECTORY, 0)?)
            .map_err(std::io::Error::other)?;
        let mut metadata = [0u8; 144];
        assert_eq!(filesystem.stat(descriptor, &mut metadata)?, 0);
        let mut directory_metadata = [0u8; 144];
        assert_eq!(
            filesystem.stat_at(-100, &path, &mut directory_metadata, 0)?,
            0
        );
        assert_eq!(&metadata[..24], &directory_metadata[..24]);
        let mut entries = [0u8; 512];
        let moved = usize::try_from(filesystem.directory_entries(descriptor, &mut entries)?)
            .map_err(std::io::Error::other)?;
        assert!(moved > 0 && moved <= entries.len());
        assert_eq!(filesystem.close(descriptor)?, 0);
        assert_eq!(
            filesystem.directory_entries(descriptor, &mut entries)?,
            -i64::from(libc::EBADF)
        );
        assert_eq!(filesystem.unlinkat(-100, &path, libc::AT_REMOVEDIR)?, 0);
        assert!(!directory.exists());
        Ok(())
    }
}
