//! Matching Linux kernel operations over an instance-private descriptor table.
//! The receiving runtime separately admits target, exact ABI and delegation.
//! The libc syscall trampoline only enters the selected kernel operation: its
//! immediate thread-local errno is restored to Linux's negative-status convention.
//! No Rust I/O error, retry, private worker descriptor or modeled stream supplies
//! a kernel result. Owned descriptors close when this namespace retires.

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
            // The CStr owns the complete initialized terminator window. The
            // synchronous syscall neither retains it nor writes its storage.
            let status = kernel_status(unsafe {
                libc::syscall(libc::SYS_openat, directory, path.as_ptr(), flags, mode)
            })?;
            if status < 0 {
                return Ok(status);
            }
            let native = i32::try_from(status)
                .map_err(|_| io::Error::other("kernel descriptor exceeds its native carrier"))?;
            // A successful openat creates exactly this owned descriptor.
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

    /// Reposition one guest descriptor. The offset is the kernel `off_t`, not
    /// a narrowed `i32`, and a failure stays the negated errno the POSIX
    /// trait already returns. Standard streams are not guest resources.
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

    /// Remove one directory entry. `AT_FDCWD` stays the host's current
    /// directory; every other directory is a guest descriptor. The path is
    /// the caller's already-terminated buffer.
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
            // The owned C string supplies every byte the synchronous call reads.
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
            // Both supported Linux kernel stat layouts fit this exact borrowed
            // window. No host pointer or adjacent guest storage is exposed.
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
            // The array owns the complete fixed kernel output window.
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
            // The kernel receives only the admitted live view, with its exact
            // byte count. Preserve partial transfers, EOF and negative errno.
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

    /// Read into the caller's already-bounded window. A short count, EOF, or
    /// negated errno is the kernel status; this table does not retry or fill
    /// past that window. Standard streams are not guest resources.
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
            // Only this initialized, authenticated input window is observable.
            // Preserve partial writes and EINTR; retries belong to guest code.
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
            // Check before removal; private standard descriptors are never guest
            // resources. Failed-close tombstones cannot revive reused OS numbers.
            self.descriptor(descriptor)?;
            let native = match self.descriptors.remove(&descriptor) {
                Some(Some(native)) => native.into_raw_fd(),
                None => -1,
                Some(None) => unreachable!("descriptor rejected its tombstone"),
            };
            // Consume ownership before close, including its error path. Linux
            // retires the descriptor before reporting late errors: no retry or
            // OwnedFd destructor may close its subsequently reused OS number.
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
            // Ask the actual kernel about a known invalid descriptor. Never
            // interpret an arbitrary guest integer in the host's descriptor table.
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
