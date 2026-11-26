//! Resolve guest path operands under retained directory handles before invoking
//! a matching Linux kernel. Descriptor operations reuse the guest descriptor table.
use super::{KernelOperand as Arg, KernelResult, LINUX_AT_FDCWD, LinuxFilesystem};
use platform_custody::directory_mappings::{DirectoryAccess, DirectoryMapping, DirectoryMappings};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::File,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
use target::LinuxFilesystemOperation as Op;

pub(super) struct Filesystem {
    roots: DirectoryMappings,
    fd_directory: std::os::fd::BorrowedFd<'static>,
    descriptors: BTreeMap<i32, DirectoryAccess>,
}

struct Entry {
    parent: File,
    leaf: CString,
    mapping: DirectoryMapping,
    root: bool,
}

impl Entry {
    fn writable(&self) -> io::Result<()> {
        if self.mapping.access() == DirectoryAccess::ReadOnly {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(())
    }

    fn mutable_name(&self) -> io::Result<()> {
        self.writable()?;
        if self.root {
            return Err(io::Error::from_raw_os_error(libc::EBUSY));
        }
        Ok(())
    }

    fn open(
        &self,
        fd_directory: std::os::fd::BorrowedFd<'_>,
        flags: i32,
        mode: libc::mode_t,
    ) -> io::Result<File> {
        // O_PATH acquires the exact object without opening a device or FIFO.
        // Check that handle before reopening it through Linux's private fd link.
        let safe = libc::O_NOFOLLOW | libc::O_CLOEXEC;
        let fd = unsafe { libc::openat(self.fd(), self.leaf.as_ptr(), libc::O_PATH | safe) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::NotFound || flags & libc::O_CREAT == 0 {
                return Err(error);
            }
            // O_EXCL guarantees a newly created regular file. A concurrent creator
            // may win the name; re-enter through the checked existing-object path.
            let fd = unsafe {
                libc::openat(
                    self.fd(),
                    self.leaf.as_ptr(),
                    flags | safe | libc::O_EXCL,
                    mode,
                )
            };
            if fd >= 0 {
                return Ok(unsafe { File::from_raw_fd(fd) });
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::AlreadyExists && flags & libc::O_EXCL == 0 {
                return self.open(fd_directory, flags & !libc::O_CREAT, mode);
            }
            return Err(error);
        }
        let pinned = unsafe { File::from_raw_fd(fd) };
        if flags & libc::O_EXCL != 0 && flags & libc::O_CREAT != 0 {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let kind = pinned.metadata()?;
        if !kind.is_file() && !kind.is_dir() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        if flags & libc::O_DIRECTORY != 0 && !kind.is_dir() {
            return Err(io::ErrorKind::NotADirectory.into());
        }
        let path = CString::new(pinned.as_raw_fd().to_string())?;
        let flags = (flags & !(libc::O_NOFOLLOW | libc::O_CREAT | libc::O_EXCL)) | libc::O_CLOEXEC;
        let fd = unsafe { libc::openat(fd_directory.as_raw_fd(), path.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn fd(&self) -> i32 {
        self.parent.as_raw_fd()
    }
}

impl Filesystem {
    pub(super) fn new(roots: DirectoryMappings) -> io::Result<Self> {
        Ok(Self {
            fd_directory: rustix_linux_procfs::proc_self_fd()?,
            roots,
            descriptors: BTreeMap::new(),
        })
    }

    pub(super) fn forget(&mut self, descriptor: i32) {
        self.descriptors.remove(&descriptor);
    }

    fn entry(&self, dir: i32, path: &CStr) -> io::Result<Entry> {
        // Absolute guest names ignore the directory argument as in openat(2).
        let bytes = path.to_bytes();
        let relative;
        let path = if bytes.starts_with(b"/") {
            bytes
        } else {
            if dir != LINUX_AT_FDCWD {
                return Err(io::ErrorKind::Unsupported.into());
            }
            if bytes.is_empty() {
                return Err(io::ErrorKind::NotFound.into());
            }
            relative = [b"/".as_slice(), bytes].concat();
            &relative
        };
        let (mapping, suffix) = self.roots.resolve_entry(path)?;
        let (parent, leaf) = mapping.bound_parent(suffix)?;
        Ok(Entry {
            parent,
            leaf: CString::new(leaf)?,
            mapping: mapping.clone(),
            root: suffix.is_empty(),
        })
    }

    pub(super) fn call(
        &mut self,
        fs: &mut LinuxFilesystem,
        number: i64,
        operands: &mut [Arg<'_>],
    ) -> io::Result<i64> {
        // Errors in the filesystem domain are ordinary negative Linux errno.
        self.perform(fs, number, operands).or_else(|error| {
            let errno = error.raw_os_error().unwrap_or(match error.kind() {
                io::ErrorKind::PermissionDenied => libc::EACCES,
                io::ErrorKind::NotFound => libc::ENOENT,
                io::ErrorKind::InvalidInput => libc::EINVAL,
                io::ErrorKind::Unsupported => libc::EOPNOTSUPP,
                _ => libc::EIO,
            });
            Ok(-i64::from(errno))
        })
    }

    fn write_allowed(&self, fd: i32) -> io::Result<()> {
        match self.descriptors.get(&fd) {
            Some(DirectoryAccess::ReadWrite) => Ok(()),
            Some(DirectoryAccess::ReadOnly) => Err(io::ErrorKind::PermissionDenied.into()),
            None => Err(io::Error::from_raw_os_error(libc::EBADF)),
        }
    }

    fn perform(
        &mut self,
        fs: &mut LinuxFilesystem,
        number: i64,
        operands: &mut [Arg<'_>],
    ) -> io::Result<i64> {
        let operation = target::TargetProfile::host_if_supported()
            .and_then(|target| Op::from_syscall(target, number))
            .ok_or(io::ErrorKind::Unsupported)?;
        use Arg::{Descriptor, Directory, Path, Value};
        match (operation, &mut *operands) {
            (Op::Openat, [Directory(dir), Path(path), Value(flags), Value(mode)]) => {
                let entry = self.entry(*dir, path)?;
                let flags = i32::try_from(*flags).map_err(|_| io::ErrorKind::InvalidInput)?;
                let supported = libc::O_ACCMODE
                    | libc::O_CREAT
                    | libc::O_EXCL
                    | libc::O_APPEND
                    | libc::O_TRUNC
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK
                    | libc::O_SYNC
                    | libc::O_DSYNC
                    | libc::O_LARGEFILE;
                if flags & !supported != 0 {
                    return Err(io::ErrorKind::Unsupported.into());
                }
                if flags & (libc::O_ACCMODE | libc::O_CREAT | libc::O_TRUNC | libc::O_APPEND) != 0 {
                    entry.writable()?;
                }
                let file = entry.open(self.fd_directory, flags, *mode as libc::mode_t)?;
                let guest = fs.issue(OwnedFd::from(file))?;
                self.descriptors
                    .insert(guest as i32, entry.mapping.access());
                Ok(guest)
            }
            (Op::Mkdirat, [Directory(dir), Path(path), Value(mode)]) => {
                let entry = self.entry(*dir, path)?;
                entry.mutable_name()?;
                status(unsafe {
                    libc::mkdirat(entry.fd(), entry.leaf.as_ptr(), *mode as libc::mode_t)
                })
            }
            (Op::Unlinkat, [Directory(dir), Path(path), Value(flags)]) => {
                let entry = self.entry(*dir, path)?;
                entry.mutable_name()?;
                status(unsafe { libc::unlinkat(entry.fd(), entry.leaf.as_ptr(), *flags as i32) })
            }
            (Op::Renameat, [Directory(d1), Path(p1), Directory(d2), Path(p2)])
            | (Op::Linkat, [Directory(d1), Path(p1), Directory(d2), Path(p2), Value(0)]) => {
                let from = self.entry(*d1, p1)?;
                let to = self.entry(*d2, p2)?;
                from.mutable_name()?;
                to.mutable_name()?;
                if from.mapping != to.mapping {
                    return Err(io::Error::from_raw_os_error(libc::EXDEV));
                }
                status(unsafe {
                    if matches!(operation, Op::Renameat) {
                        libc::renameat(from.fd(), from.leaf.as_ptr(), to.fd(), to.leaf.as_ptr())
                    } else {
                        libc::linkat(from.fd(), from.leaf.as_ptr(), to.fd(), to.leaf.as_ptr(), 0)
                    }
                })
            }
            (Op::Readlinkat, [Directory(dir), Path(path), Arg::OutputView(out)]) => {
                let entry = self.entry(*dir, path)?;
                let count = unsafe {
                    libc::readlinkat(
                        entry.fd(),
                        entry.leaf.as_ptr(),
                        out.as_mut_ptr().cast(),
                        out.len(),
                    )
                };
                if count < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(count as i64)
            }
            (Op::Newfstatat, [Directory(dir), Path(path), Arg::Output(out), Value(flags)]) => {
                let entry = self.entry(*dir, path)?;
                if out.len() < size_of::<libc::stat>()
                    || *flags & !i64::from(libc::AT_SYMLINK_NOFOLLOW) != 0
                {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
                let mut metadata = std::mem::MaybeUninit::<libc::stat>::zeroed();
                status(unsafe {
                    libc::fstatat(
                        entry.fd(),
                        entry.leaf.as_ptr(),
                        metadata.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                })?;
                // Following a leaf symlink would let it leave the granted tree.
                if *flags == 0
                    && unsafe { (*metadata.as_ptr()).st_mode } & libc::S_IFMT == libc::S_IFLNK
                {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        metadata.as_ptr().cast::<u8>(),
                        out.as_mut_ptr(),
                        size_of::<libc::stat>(),
                    );
                }
                Ok(0)
            }
            (Op::Dup, [Descriptor(original)]) => {
                let grant = self
                    .descriptors
                    .get(original)
                    .copied()
                    .ok_or_else(|| io::Error::from_raw_os_error(libc::EBADF))?;
                let guest = fs.raw_call(number, operands, KernelResult::Descriptor)?;
                if guest >= 0 {
                    self.descriptors.insert(guest as i32, grant);
                }
                Ok(guest)
            }
            (Op::Fchmod | Op::Ftruncate, [Descriptor(fd), Value(_)])
            | (Op::Fchown, [Descriptor(fd), Value(_), Value(_)])
            | (Op::Pwrite64, [Descriptor(fd), Arg::InputView(_), Value(_)]) => {
                self.write_allowed(*fd)?;
                fs.raw_call(number, operands, KernelResult::Status)
            }
            (Op::Utimensat, [Descriptor(fd), Value(0), Arg::Input(times), Value(0)])
                if times.len() == 2 * size_of::<libc::timespec>() =>
            {
                self.write_allowed(*fd)?;
                fs.raw_call(number, operands, KernelResult::Status)
            }
            (Op::Fstat, [Descriptor(_), Arg::Output(out)])
                if out.len() >= size_of::<libc::stat>() =>
            {
                fs.raw_call(number, operands, KernelResult::Status)
            }
            (Op::Getdents64, [Descriptor(_), Arg::OutputView(_)])
            | (Op::Lseek, [Descriptor(_), Value(_), Value(_)])
            | (Op::Pread64, [Descriptor(_), Arg::OutputView(_), Value(_)])
            | (Op::Fsync | Op::Fdatasync, [Descriptor(_)])
            | (Op::Flock, [Descriptor(_), Value(_)]) => {
                fs.raw_call(number, operands, KernelResult::Status)
            }
            _ => Err(io::ErrorKind::Unsupported.into()),
        }
    }
}

fn status(value: i32) -> io::Result<i64> {
    if value < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(i64::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_custody::temporary_directory::TemporaryDirectory;

    #[test]
    fn a_readonly_fd_does_not_authorize_metadata_writes_or_symlink_escape() {
        let temp = TemporaryDirectory::create("guest-fd-access", false).unwrap();
        let directory = temp.path().join("directory");
        let other = temp.path().join("other");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("inside"), b"mapped").unwrap();
        std::fs::write(&other, b"outside").unwrap();
        std::os::unix::fs::symlink(&other, directory.join("escape")).unwrap();
        let mut grants = DirectoryMappings::default();
        grants
            .grant(&directory, "/view", DirectoryAccess::ReadOnly)
            .unwrap();
        let mut fs = LinuxFilesystem::mapped(grants).unwrap();
        let open = |fs: &mut LinuxFilesystem, path: &CStr, flags: i32| {
            fs.call(
                libc::SYS_openat,
                &mut [
                    Arg::Directory(LINUX_AT_FDCWD),
                    Arg::Path(path),
                    Arg::Value(i64::from(flags)),
                    Arg::Value(0o600),
                ],
                KernelResult::Descriptor,
            )
            .unwrap()
        };
        let fd = open(&mut fs, c"/view/inside", libc::O_RDONLY) as i32;
        assert!(fd >= 0);
        let mut bytes = [0; 6];
        assert_eq!(fs.read(fd, &mut bytes).unwrap(), 6);
        assert_eq!(&bytes, b"mapped");
        assert_eq!(
            fs.call(
                libc::SYS_fchmod,
                &mut [Arg::Descriptor(fd), Arg::Value(0)],
                KernelResult::Status
            )
            .unwrap(),
            -i64::from(libc::EACCES)
        );
        let duplicate = fs
            .call(
                libc::SYS_dup,
                &mut [Arg::Descriptor(fd)],
                KernelResult::Descriptor,
            )
            .unwrap() as i32;
        assert!(duplicate >= 0);
        assert_eq!(
            fs.call(
                libc::SYS_ftruncate,
                &mut [Arg::Descriptor(duplicate), Arg::Value(1)],
                KernelResult::Status
            )
            .unwrap(),
            -i64::from(libc::EACCES)
        );
        assert_eq!(
            open(&mut fs, c"/view/escape", libc::O_RDONLY),
            -i64::from(libc::EACCES)
        );
        let ambient = CString::new(other.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(
            open(&mut fs, &ambient, libc::O_RDONLY),
            -i64::from(libc::EACCES)
        );
        assert_eq!(
            open(&mut fs, c"/view/inside", libc::O_WRONLY | libc::O_TRUNC),
            -i64::from(libc::EACCES)
        );
        assert_eq!(std::fs::read(directory.join("inside")).unwrap(), b"mapped");
    }
}
