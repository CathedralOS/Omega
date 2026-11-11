/*
We execute Linux kernel filesystem operations over an instance-private guest
descriptor table (`LinuxFilesystem`).

The receiving runtime separately admits target, exact ABI, and delegation, then
hands us one exact system call number with its operands already classified:
directory and descriptor operands translate through the guest table, and
buffers pass their own addresses (and lengths, for byte views). Our
`libc::syscall` trampoline enters the selected kernel operation and translates
its immediate thread-local `errno` back into Linux's negative-status convention.
No Rust I/O error, retry loop, private worker descriptor, or modeled standard
stream (`0..=2`) supplies a kernel result.

A namespace may be rooted at a directory: `AT_FDCWD` then names that directory
rather than this process's working directory, so concurrent instances in one
process never resolve each other's relative paths. Absolute paths still name
the host's namespace, exactly as they do for the native program.

When `close` fails after retiring the OS descriptor, we record a `None`
tombstone so neither retries nor `OwnedFd` destructors can close a subsequently
reused OS descriptor number.
*/

use std::{ffi::CStr, io};

/// The kernel's "relative to the working directory" directory operand.
pub const LINUX_AT_FDCWD: i32 = -100;

/// One classified system call operand, in kernel argument order.
pub enum KernelOperand<'a> {
    /// A directory operand: `AT_FDCWD` names the namespace root, any other
    /// value a guest descriptor.
    Directory(i32),
    /// A guest descriptor.
    Descriptor(i32),
    /// A plain integer passed through unchanged (flags, modes, offsets, ids).
    Value(i64),
    /// A NUL-terminated path, passed by address.
    Path(&'a CStr),
    /// Caller bytes the kernel only reads, passed by address.
    Input(&'a [u8]),
    /// Caller bytes the kernel may write, passed by address.
    Output(&'a mut [u8]),
    /// A read-only byte view, passed as address and length.
    InputView(&'a [u8]),
    /// A writable byte view, passed as address and length.
    OutputView(&'a mut [u8]),
}

/// What a successful nonnegative kernel status means.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KernelResult {
    /// The status is returned unchanged.
    Status,
    /// The status is a new OS descriptor, registered under a fresh guest number.
    Descriptor,
}

#[derive(Default)]
pub struct LinuxFilesystem {
    #[cfg(target_os = "linux")]
    root: Option<std::os::fd::OwnedFd>,
    #[cfg(target_os = "linux")]
    descriptors: std::collections::BTreeMap<i32, Option<std::os::fd::OwnedFd>>,
}

impl LinuxFilesystem {
    /// A namespace whose `AT_FDCWD` is `root` instead of the process directory.
    pub fn rooted(root: &std::path::Path) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let directory = std::fs::File::open(root)?;
            if !directory.metadata()?.is_dir() {
                return Err(io::Error::other(
                    "filesystem namespace root is not a directory",
                ));
            }
            Ok(Self {
                root: Some(directory.into()),
                descriptors: std::collections::BTreeMap::new(),
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = root;
            Err(unsupported())
        }
    }

    /// Enter system call `number` with `operands` in kernel argument order.
    pub fn call(
        &mut self,
        number: i64,
        operands: &mut [KernelOperand<'_>],
        result: KernelResult,
    ) -> io::Result<i64> {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::FromRawFd;
            let mut arguments: Vec<libc::c_long> = Vec::with_capacity(6);
            for operand in operands.iter_mut() {
                match operand {
                    KernelOperand::Directory(directory) => {
                        arguments.push(libc::c_long::from(self.directory(*directory)?));
                    }
                    KernelOperand::Descriptor(descriptor) => {
                        arguments.push(libc::c_long::from(self.descriptor(*descriptor)?));
                    }
                    KernelOperand::Value(value) => arguments.push(*value as libc::c_long),
                    KernelOperand::Path(path) => arguments.push(path.as_ptr() as libc::c_long),
                    KernelOperand::Input(bytes) => arguments.push(bytes.as_ptr() as libc::c_long),
                    KernelOperand::Output(bytes) => {
                        arguments.push(bytes.as_mut_ptr() as libc::c_long);
                    }
                    KernelOperand::InputView(bytes) => {
                        arguments.push(bytes.as_ptr() as libc::c_long);
                        arguments.push(bytes.len() as libc::c_long);
                    }
                    KernelOperand::OutputView(bytes) => {
                        arguments.push(bytes.as_mut_ptr() as libc::c_long);
                        arguments.push(bytes.len() as libc::c_long);
                    }
                }
            }
            if arguments.len() > 6 {
                return Err(io::Error::other(
                    "Linux system calls take at most six arguments",
                ));
            }
            arguments.resize(6, 0);
            let number = libc::c_long::try_from(number)
                .map_err(|_| io::Error::other("system call number exceeds its carrier"))?;
            let status = kernel_status(unsafe {
                libc::syscall(
                    number,
                    arguments[0],
                    arguments[1],
                    arguments[2],
                    arguments[3],
                    arguments[4],
                    arguments[5],
                )
            })?;
            if status < 0 || result == KernelResult::Status {
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
            let _ = (number, operands, result);
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
    fn directory(&self, guest: i32) -> io::Result<i32> {
        use std::os::fd::AsRawFd;
        if guest != LINUX_AT_FDCWD {
            return self.descriptor(guest);
        }
        Ok(self
            .root
            .as_ref()
            .map_or(libc::AT_FDCWD, |root| root.as_raw_fd()))
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
    use super::{KernelOperand as Operand, KernelResult, LINUX_AT_FDCWD, LinuxFilesystem};
    use std::ffi::CString;

    fn scratch(label: &str) -> std::io::Result<std::path::PathBuf> {
        let directory = std::env::temp_dir().join(format!(
            "omega-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(std::io::Error::other)?
                .as_nanos()
        ));
        Ok(directory)
    }

    #[test]
    fn directory_lifecycle_uses_private_descriptors() -> std::io::Result<()> {
        let directory = scratch("directory")?;
        let path = CString::new(directory.as_os_str().as_encoded_bytes())?;
        let mut filesystem = LinuxFilesystem::default();
        let mkdir = |filesystem: &mut LinuxFilesystem| {
            filesystem.call(
                libc::SYS_mkdirat,
                &mut [
                    Operand::Directory(LINUX_AT_FDCWD),
                    Operand::Path(&path),
                    Operand::Value(0o700),
                ],
                KernelResult::Status,
            )
        };
        assert_eq!(mkdir(&mut filesystem)?, 0);
        assert_eq!(mkdir(&mut filesystem)?, -i64::from(libc::EEXIST));
        let descriptor = i32::try_from(filesystem.call(
            libc::SYS_openat,
            &mut [
                Operand::Directory(LINUX_AT_FDCWD),
                Operand::Path(&path),
                Operand::Value(i64::from(libc::O_DIRECTORY)),
                Operand::Value(0),
            ],
            KernelResult::Descriptor,
        )?)
        .map_err(std::io::Error::other)?;
        let mut entries = [0u8; 512];
        let moved = filesystem.call(
            libc::SYS_getdents64,
            &mut [
                Operand::Descriptor(descriptor),
                Operand::OutputView(&mut entries),
            ],
            KernelResult::Status,
        )?;
        assert!(moved > 0 && moved <= 512);
        assert_eq!(filesystem.close(descriptor)?, 0);
        assert_eq!(
            filesystem.call(
                libc::SYS_getdents64,
                &mut [
                    Operand::Descriptor(descriptor),
                    Operand::OutputView(&mut entries),
                ],
                KernelResult::Status,
            )?,
            -i64::from(libc::EBADF)
        );
        assert_eq!(
            filesystem.call(
                libc::SYS_unlinkat,
                &mut [
                    Operand::Directory(LINUX_AT_FDCWD),
                    Operand::Path(&path),
                    Operand::Value(i64::from(libc::AT_REMOVEDIR)),
                ],
                KernelResult::Status,
            )?,
            0
        );
        assert!(!directory.exists());
        Ok(())
    }

    #[test]
    fn rooted_namespace_resolves_relative_paths_beneath_its_root() -> std::io::Result<()> {
        let root = scratch("root")?;
        std::fs::create_dir(&root)?;
        let mut filesystem = LinuxFilesystem::rooted(&root)?;
        let link = CString::new("link")?;
        let target = CString::new("target-text")?;
        assert_eq!(
            filesystem.call(
                libc::SYS_symlinkat,
                &mut [
                    Operand::Path(&target),
                    Operand::Directory(LINUX_AT_FDCWD),
                    Operand::Path(&link),
                ],
                KernelResult::Status,
            )?,
            0
        );
        let mut read = [0u8; 4];
        assert_eq!(
            filesystem.call(
                libc::SYS_readlinkat,
                &mut [
                    Operand::Directory(LINUX_AT_FDCWD),
                    Operand::Path(&link),
                    Operand::OutputView(&mut read),
                ],
                KernelResult::Status,
            )?,
            4
        );
        assert_eq!(&read, b"targ");
        assert_eq!(
            std::fs::read_link(root.join("link"))?,
            std::path::Path::new("target-text")
        );
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }
}
