//! Install and observe ordinary files in the same namespace used by guest calls.
use super::{Contents, LINUX_AT_FDCWD, O_CREAT, O_EXCL, O_WRONLY, PrivateLinuxFilesystem};
use std::{collections::BTreeMap, io, sync::Arc};

impl PrivateLinuxFilesystem {
    /// Build creates only regular files and their parent directories. Use the
    /// guest filesystem's ordinary allocation, capacity and permission rules.
    pub fn with_initial_files(files: &BTreeMap<Vec<u8>, Arc<[u8]>>) -> io::Result<Self> {
        let mut fs = Self::new();
        for (path, bytes) in files {
            if path.contains(&0)
                || path
                    .split(|&byte| byte == b'/')
                    .any(|p| p.is_empty() || p == b"." || p == b"..")
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid initial file path",
                ));
            }
            for (index, _) in path.iter().enumerate().filter(|(_, b)| **b == b'/') {
                let parent = &path[..index];
                match fs.mkdirat(LINUX_AT_FDCWD, parent, 0o755) {
                    Ok(()) => (),
                    Err(super::EEXIST)
                        if fs
                            .existing(LINUX_AT_FDCWD, parent, false)
                            .is_ok_and(|node| fs.is_directory(node)) =>
                    {
                        ()
                    }
                    Err(error) => return Err(errno(error)),
                }
            }
            let fd = fs
                .openat(LINUX_AT_FDCWD, path, O_CREAT | O_EXCL | O_WRONLY, 0o644)
                .map_err(errno)? as i32;
            fs.transfer_out(fd, bytes, None).map_err(errno)?;
            fs.close_guest(fd).map_err(errno)?;
        }
        Ok(fs)
    }

    /// Preserve absent versus empty and resolve links inside this invocation.
    pub fn file_bytes(&self, path: &[u8], remaining: usize) -> io::Result<Option<Vec<u8>>> {
        let node = match self.existing(LINUX_AT_FDCWD, path, true) {
            Ok(node) => node,
            Err(super::ENOENT | super::ENOTDIR) => return Ok(None),
            Err(error) => return Err(errno(error)),
        };
        if !self.permits(node, 4) {
            return Err(errno(super::EACCES));
        }
        let Contents::File(bytes) = &self.node(node).map_err(errno)?.contents else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "declared output is not a regular file",
            ));
        };
        if bytes.len() > remaining {
            return Err(io::Error::other(
                "declared files exceed their capture allowance",
            ));
        }
        Ok(Some(bytes.clone()))
    }
}

fn errno(errno: i64) -> io::Error {
    io::Error::from_raw_os_error(errno as i32)
}
