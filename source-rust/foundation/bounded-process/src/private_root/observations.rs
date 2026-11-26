//! Retain the guest's private mount until the sponsor has read the declared files.
//! The child sends its root before exec. The executable receives neither socket.
use super::PrivateFilesystemPurpose;
use std::{collections::BTreeMap, io, sync::Arc};

/// Byte paths are relative to the root of the new filesystem. Initial files are
/// ordinary data. An output name asks only for presence and regular-file bytes.
#[derive(Debug, Default)]
pub struct PrivateFilesystemFiles {
    pub initial: BTreeMap<Vec<u8>, Arc<[u8]>>,
    pub outputs: Vec<Vec<u8>>,
}

pub struct PrivateFilesystemOutputs {
    paths: Vec<Vec<u8>>,
    #[cfg(target_os = "linux")]
    receiver: Option<std::os::unix::net::UnixDatagram>,
}

impl PrivateFilesystemOutputs {
    /// Create the outer sponsor's observation channel. A trusted namespace
    /// supervisor forwards its captured root before exiting. The sponsor
    /// can then wait for the complete namespace to disappear before reading.
    #[cfg(target_os = "linux")]
    pub fn channel(paths: Vec<Vec<u8>>) -> io::Result<(Self, std::os::fd::OwnedFd)> {
        let (sender, receiver) = std::os::unix::net::UnixDatagram::pair()?;
        Ok((
            Self {
                paths,
                receiver: Some(receiver),
            },
            sender.into(),
        ))
    }

    #[cfg(target_os = "linux")]
    pub fn forward_root(self, destination: &impl std::os::fd::AsFd) -> io::Result<()> {
        if !self.paths.is_empty() {
            let root = linux::receive_root(self.receiver.ok_or(io::ErrorKind::NotConnected)?)?;
            linux::send_root(destination, &root)?;
        }
        Ok(())
    }

    /// Call after the bounded child and its descendants have been reaped.
    pub fn collect(self, byte_limit: usize) -> io::Result<Vec<Option<Vec<u8>>>> {
        if self.paths.is_empty() {
            return Ok(Vec::new());
        }
        #[cfg(target_os = "linux")]
        {
            use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};
            use std::io::Read;
            let socket = self.receiver.ok_or(io::ErrorKind::NotConnected)?;
            let root = linux::receive_root(socket)?;
            let mut remaining = byte_limit;
            let mut files = Vec::with_capacity(self.paths.len());
            for path in self.paths {
                let file = match openat2(
                    &root,
                    path.as_slice(),
                    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
                    Mode::empty(),
                    ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
                ) {
                    Ok(fd) => std::fs::File::from(fd),
                    Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) => {
                        files.push(None);
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                let metadata = file.metadata()?;
                if !metadata.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "declared output is not a regular file",
                    ));
                }
                if metadata.len() > remaining as u64 {
                    return Err(io::Error::other(
                        "declared files exceed their capture allowance",
                    ));
                }
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(metadata.len() as usize)
                    .map_err(io::Error::other)?;
                file.take((remaining as u64).saturating_add(1))
                    .read_to_end(&mut bytes)?;
                remaining = remaining.checked_sub(bytes.len()).ok_or_else(|| {
                    io::Error::other("declared files exceed their capture allowance")
                })?;
                files.push(Some(bytes));
            }
            Ok(files)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = byte_limit;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this host cannot observe a private test filesystem",
            ))
        }
    }
}

#[derive(Debug)]
pub(crate) struct Setup {
    #[cfg(target_os = "linux")]
    pub(super) purpose: PrivateFilesystemPurpose,
    #[cfg(target_os = "linux")]
    pub(super) scratch: std::path::PathBuf,
    #[cfg(target_os = "linux")]
    pub(super) contents: linux::Contents,
}

impl Setup {
    pub(crate) fn new(
        _scratch: &std::path::Path,
        files: PrivateFilesystemFiles,
        purpose: PrivateFilesystemPurpose,
    ) -> io::Result<(Self, PrivateFilesystemOutputs)> {
        for path in files.initial.keys().chain(&files.outputs) {
            if path.len() >= 4096
                || path.contains(&0)
                || !path.split(|&byte| byte == b'/').all(|part| {
                    !part.is_empty() && part.len() <= 255 && part != b"." && part != b".."
                })
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "private file needs a canonical relative byte path",
                ));
            }
        }
        #[cfg(target_os = "linux")]
        let (contents, receiver) = linux::Contents::new(
            files.initial,
            &files.outputs,
            purpose != PrivateFilesystemPurpose::NativeTest,
        )?;
        #[cfg(not(target_os = "linux"))]
        let _ = purpose;
        Ok((
            Self {
                #[cfg(target_os = "linux")]
                scratch: _scratch.to_owned(),
                #[cfg(target_os = "linux")]
                purpose,
                #[cfg(target_os = "linux")]
                contents,
            },
            PrivateFilesystemOutputs {
                paths: files.outputs,
                #[cfg(target_os = "linux")]
                receiver,
            },
        ))
    }
}

#[cfg(target_os = "linux")]
pub(super) mod linux {
    use super::*;
    use rustix::{
        fs::{Mode, OFlags},
        net::{
            RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
            SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
        },
    };
    use std::{
        collections::BTreeSet,
        ffi::CString,
        io::{IoSlice, IoSliceMut},
        mem::MaybeUninit,
        os::{
            fd::{AsFd, OwnedFd},
            unix::net::UnixDatagram,
        },
    };

    #[derive(Debug)]
    pub(in super::super) struct Contents {
        directories: Vec<CString>,
        files: Vec<(CString, Arc<[u8]>, bool)>,
        sender: Option<UnixDatagram>,
    }

    impl Contents {
        pub(super) fn new(
            files: BTreeMap<Vec<u8>, Arc<[u8]>>,
            outputs: &[Vec<u8>],
            protect_inputs: bool,
        ) -> io::Result<(Self, Option<UnixDatagram>)> {
            let mut parents = BTreeSet::new();
            let mut total = 0usize;
            for (path, bytes) in &files {
                total = total
                    .checked_add(bytes.len())
                    .ok_or(io::ErrorKind::InvalidInput)?;
                if total > 64 << 20 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "initial files exceed the private filesystem's capacity",
                    ));
                }
                for (index, &byte) in path.iter().enumerate() {
                    if byte == b'/' {
                        let parent = &path[..index];
                        if files.contains_key(parent) {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "an initial file is also a parent directory",
                            ));
                        }
                        parents.insert(parent.to_owned());
                    }
                }
            }
            if protect_inputs {
                for path in outputs {
                    for (index, &byte) in path.iter().enumerate() {
                        if byte == b'/' {
                            parents.insert(path[..index].to_owned());
                        }
                    }
                }
            }
            let directories = parents
                .into_iter()
                .map(CString::new)
                .collect::<Result<_, _>>()?;
            let files = files
                .into_iter()
                .map(|(path, bytes)| {
                    let read_only = protect_inputs && !outputs.contains(&path);
                    Ok((CString::new(path)?, bytes, read_only))
                })
                .collect::<io::Result<_>>()?;
            let (sender, receiver) = if !outputs.is_empty() {
                let (sender, receiver) = UnixDatagram::pair()?;
                (Some(sender), Some(receiver))
            } else {
                (None, None)
            };
            Ok((
                Self {
                    directories,
                    files,
                    sender,
                },
                receiver,
            ))
        }

        /// All paths and message storage exist before fork. Only trusted setup
        /// code has run since entering the empty root.
        pub(in super::super) fn install(&self) -> io::Result<()> {
            for directory in &self.directories {
                rustix::fs::mkdir(directory.as_c_str(), Mode::from_raw_mode(0o755))?;
            }
            for (path, contents, read_only) in &self.files {
                let file = rustix::fs::open(
                    path.as_c_str(),
                    OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
                    Mode::from_raw_mode(0o644),
                )?;
                let mut bytes = contents.as_ref();
                while !bytes.is_empty() {
                    match rustix::io::write(&file, bytes) {
                        Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                        Ok(count) => bytes = &bytes[count..],
                        Err(rustix::io::Errno::INTR) => continue,
                        Err(error) => return Err(error.into()),
                    }
                }
                if *read_only {
                    // Binding the captured file read-only also prevents the
                    // private-root owner from reopening it after a chmod.
                    let file = path.as_ptr();
                    unsafe {
                        super::super::check_mount(file, file, libc::MS_BIND)?;
                        super::super::check_mount(
                            std::ptr::null(),
                            file,
                            libc::MS_BIND
                                | libc::MS_REMOUNT
                                | libc::MS_RDONLY
                                | libc::MS_NOSUID
                                | libc::MS_NODEV
                                | libc::MS_NOEXEC,
                        )?;
                    }
                }
            }
            if let Some(sender) = &self.sender {
                let root = rustix::fs::open(
                    c"/",
                    OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                )?;
                send_root(sender, &root)?;
            }
            Ok(())
        }
    }

    pub(super) fn send_root(sender: &impl AsFd, root: &impl AsFd) -> io::Result<()> {
        let fds = [root.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let sent = sendmsg(
            sender,
            &[IoSlice::new(b"R")],
            &mut control,
            SendFlags::NOSIGNAL,
        )?;
        if sent != 1 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        Ok(())
    }

    pub(super) fn receive_root(socket: UnixDatagram) -> io::Result<OwnedFd> {
        let mut byte = [0];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let message = recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut byte)],
            &mut control,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
        )?;
        if message.bytes != 1
            || byte != *b"R"
            || message
                .flags
                .intersects(ReturnFlags::CTRUNC | ReturnFlags::TRUNC)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut root = None;
        for message in control.drain() {
            let RecvAncillaryMessage::ScmRights(fds) = message else {
                return Err(io::ErrorKind::InvalidData.into());
            };
            for fd in fds {
                if root.replace(fd).is_some() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
            }
        }
        root.ok_or_else(|| io::ErrorKind::InvalidData.into())
    }
}

#[cfg(test)]
mod tests {
    use super::{PrivateFilesystemFiles, PrivateFilesystemPurpose, Setup};
    use std::{io, path::Path};

    #[test]
    fn observations_require_a_name_relative_to_the_invocation() {
        for path in [b"/result".as_slice(), b"../result", b"a//b", b".", b""] {
            let fixture = PrivateFilesystemFiles {
                outputs: vec![path.to_vec()],
                ..Default::default()
            };
            assert!(
                matches!(Setup::new(Path::new("."), fixture, PrivateFilesystemPurpose::NativeTest), Err(e) if e.kind() == io::ErrorKind::InvalidInput)
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_seed_file_cannot_also_be_a_parent_directory() {
        let fixture = PrivateFilesystemFiles {
            initial: [b"a".to_vec(), b"a/b".to_vec()]
                .into_iter()
                .map(|name| (name, std::sync::Arc::from([])))
                .collect(),
            ..Default::default()
        };
        assert!(
            matches!(Setup::new(Path::new("."), fixture, PrivateFilesystemPurpose::NativeTest), Err(e) if e.kind() == io::ErrorKind::InvalidInput)
        );
    }
}
