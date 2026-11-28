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
    /// The grant each issued guest descriptor was opened under.
    descriptors: BTreeMap<i32, DirectoryMapping>,
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

    /// The exact regular file or directory this entry names, held by an
    /// `O_PATH` handle. A symbolic link, device or FIFO is refused.
    fn pinned(&self) -> io::Result<File> {
        let fd = unsafe {
            libc::openat(
                self.fd(),
                self.leaf.as_ptr(),
                libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let pinned = unsafe { File::from_raw_fd(fd) };
        let kind = pinned.metadata()?;
        if !kind.is_file() && !kind.is_dir() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(pinned)
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

    fn entry(&self, fs: &LinuxFilesystem, dir: i32, path: &CStr) -> io::Result<Entry> {
        // Absolute guest names ignore the directory argument as in openat(2).
        let bytes = path.to_bytes();
        let relative;
        let path = if bytes.starts_with(b"/") {
            bytes
        } else {
            if dir != LINUX_AT_FDCWD {
                return self.entry_beneath(fs, dir, bytes);
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

    /// A single normal name beneath a directory descriptor that a confined open
    /// issued, as a recursive removal walks it. The descriptor already names
    /// an object inside its grant, and one component that is neither `.` nor
    /// `..` and opened without following links cannot leave it.
    fn entry_beneath(&self, fs: &LinuxFilesystem, dir: i32, name: &[u8]) -> io::Result<Entry> {
        let mapping = self
            .descriptors
            .get(&dir)
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EBADF))?;
        if name.is_empty() || name == b"." || name == b".." || name.contains(&b'/') {
            return Err(io::ErrorKind::Unsupported.into());
        }
        let native = fs.descriptor(dir)?;
        let borrowed = unsafe { std::os::fd::BorrowedFd::borrow_raw(native) };
        Ok(Entry {
            parent: File::from(borrowed.try_clone_to_owned()?),
            leaf: CString::new(name)?,
            mapping: mapping.clone(),
            root: false,
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
        match self.descriptors.get(&fd).map(DirectoryMapping::access) {
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
                let entry = self.entry(fs, *dir, path)?;
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
                self.descriptors.insert(guest as i32, entry.mapping.clone());
                Ok(guest)
            }
            (Op::Mkdirat, [Directory(dir), Path(path), Value(mode)]) => {
                let entry = self.entry(fs, *dir, path)?;
                entry.mutable_name()?;
                status(unsafe {
                    libc::mkdirat(entry.fd(), entry.leaf.as_ptr(), *mode as libc::mode_t)
                })
            }
            (Op::Unlinkat, [Directory(dir), Path(path), Value(flags)]) => {
                let entry = self.entry(fs, *dir, path)?;
                entry.mutable_name()?;
                status(unsafe { libc::unlinkat(entry.fd(), entry.leaf.as_ptr(), *flags as i32) })
            }
            (Op::Renameat, [Directory(d1), Path(p1), Directory(d2), Path(p2)])
            | (Op::Linkat, [Directory(d1), Path(p1), Directory(d2), Path(p2), Value(0)]) => {
                let from = self.entry(fs, *d1, p1)?;
                let to = self.entry(fs, *d2, p2)?;
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
            (Op::Fchmodat, [Directory(dir), Path(path), Value(mode)]) => {
                let entry = self.entry(fs, *dir, path)?;
                entry.writable()?;
                // Pin the exact object without following a leaf link, then
                // change it through Linux's private fd link.
                let pinned = entry.pinned()?;
                let link = CString::new(pinned.as_raw_fd().to_string())?;
                status(unsafe {
                    libc::fchmodat(
                        self.fd_directory.as_raw_fd(),
                        link.as_ptr(),
                        *mode as libc::mode_t,
                        0,
                    )
                })
            }
            (Op::Readlinkat, [Directory(dir), Path(path), Arg::OutputView(out)]) => {
                let entry = self.entry(fs, *dir, path)?;
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
                let entry = self.entry(fs, *dir, path)?;
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
                    .cloned()
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
        let chmod = |fs: &mut LinuxFilesystem, path: &CStr| {
            fs.call(
                libc::SYS_fchmodat,
                &mut [
                    Arg::Directory(LINUX_AT_FDCWD),
                    Arg::Path(path),
                    Arg::Value(0o777),
                ],
                KernelResult::Status,
            )
            .unwrap()
        };
        assert_eq!(chmod(&mut fs, c"/view/inside"), -i64::from(libc::EACCES));
    }

    #[test]
    fn a_mapped_chmod_changes_only_the_granted_object() {
        use std::os::unix::fs::PermissionsExt;
        let temp = TemporaryDirectory::create("guest-chmod", false).unwrap();
        let directory = temp.path().join("directory");
        let other = temp.path().join("other");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("inside"), b"mapped").unwrap();
        std::fs::write(&other, b"outside").unwrap();
        std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&other, directory.join("escape")).unwrap();
        let mut grants = DirectoryMappings::default();
        grants
            .grant(&directory, "/", DirectoryAccess::ReadWrite)
            .unwrap();
        let mut fs = LinuxFilesystem::mapped(grants).unwrap();
        let mut chmod = |path: &CStr, mode: i64| {
            fs.call(
                libc::SYS_fchmodat,
                &mut [
                    Arg::Directory(LINUX_AT_FDCWD),
                    Arg::Path(path),
                    Arg::Value(mode),
                ],
                KernelResult::Status,
            )
            .unwrap()
        };
        assert_eq!(chmod(c"/inside", 0o640), 0);
        let mode = |path: &std::path::Path| {
            std::fs::symlink_metadata(path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode(&directory.join("inside")), 0o640);
        assert!(chmod(c"/escape", 0o777) < 0);
        assert_eq!(mode(&other), 0o600);
    }
    #[test]
    fn a_directory_descriptor_names_only_its_own_children() {
        let temp = TemporaryDirectory::create("guest-directory-relative", false).unwrap();
        let directory = temp.path().join("directory");
        std::fs::create_dir_all(directory.join("tree")).unwrap();
        std::fs::write(directory.join("tree/leaf"), b"x").unwrap();
        std::fs::write(temp.path().join("outside"), b"keep").unwrap();
        let mut grants = DirectoryMappings::default();
        grants
            .grant(&directory, "/", DirectoryAccess::ReadWrite)
            .unwrap();
        let mut fs = LinuxFilesystem::mapped(grants).unwrap();
        let tree = fs
            .call(
                libc::SYS_openat,
                &mut [
                    Arg::Directory(LINUX_AT_FDCWD),
                    Arg::Path(c"/tree"),
                    Arg::Value(i64::from(libc::O_DIRECTORY)),
                    Arg::Value(0),
                ],
                KernelResult::Descriptor,
            )
            .unwrap() as i32;
        assert!(tree >= 0);
        let mut unlink = |name: &CStr, flags: i32| {
            fs.call(
                libc::SYS_unlinkat,
                &mut [
                    Arg::Directory(tree),
                    Arg::Path(name),
                    Arg::Value(i64::from(flags)),
                ],
                KernelResult::Status,
            )
            .unwrap()
        };
        assert_eq!(
            unlink(c"..", libc::AT_REMOVEDIR),
            -i64::from(libc::EOPNOTSUPP)
        );
        assert_eq!(unlink(c"../../outside", 0), -i64::from(libc::EOPNOTSUPP));
        assert_eq!(unlink(c"leaf", 0), 0);
        assert!(!directory.join("tree/leaf").exists());
        assert_eq!(std::fs::read(temp.path().join("outside")).unwrap(), b"keep");
        // A descriptor this namespace never issued names nothing.
        let forged = fs
            .call(
                libc::SYS_unlinkat,
                &mut [
                    Arg::Directory(tree + 100),
                    Arg::Path(c"leaf"),
                    Arg::Value(0),
                ],
                KernelResult::Status,
            )
            .unwrap();
        assert_eq!(forged, -i64::from(libc::EBADF));
    }

    #[test]
    fn opening_the_mount_root_lists_the_retained_directory() {
        let temp = TemporaryDirectory::create("guest-directory-list", false).unwrap();
        std::fs::write(temp.path().join("ledger-entry"), b"content").unwrap();
        let mut grants = DirectoryMappings::default();
        grants
            .grant(temp.path(), "/ledger", DirectoryAccess::ReadOnly)
            .unwrap();
        let mut fs = LinuxFilesystem::mapped(grants).unwrap();
        let directory = fs
            .call(
                libc::SYS_openat,
                &mut [
                    Arg::Directory(LINUX_AT_FDCWD),
                    Arg::Path(c"/ledger"),
                    Arg::Value(i64::from(libc::O_DIRECTORY)),
                    Arg::Value(0),
                ],
                KernelResult::Descriptor,
            )
            .unwrap();
        assert!(directory >= 0);
        let mut entries = [0; 512];
        let count = fs
            .call(
                libc::SYS_getdents64,
                &mut [
                    Arg::Descriptor(directory as i32),
                    Arg::OutputView(&mut entries),
                ],
                KernelResult::Status,
            )
            .unwrap();
        assert!(count > 0);
        assert!(
            entries[..count as usize]
                .windows(b"ledger-entry\0".len())
                .any(|name| name == b"ledger-entry\0")
        );
    }
}
