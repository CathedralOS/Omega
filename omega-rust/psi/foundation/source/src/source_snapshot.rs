//! Exact, non-following access to an immutable source inventory. Storage stays
//! with acquisition's owner; this borrowed protocol neither copies its model
//! nor gives source consumers access to the host. Compiler-following lookup in
//! `CapturedSourceTree` is deliberately a different operation.

use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotEntry<'storage> {
    Directory,
    File(&'storage [u8]),
    Symlink(&'storage [u8]),
}

/// Only logical kind and extent belong to the captured metadata protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotMetadata {
    Directory,
    File { byte_length: u64 },
    Symlink { target_byte_length: u64 },
}

/// Implementations borrow existing immutable storage. Paths are canonical
/// UTF-8 slash-separated relative bytes with explicit directory parents;
/// enumeration visits each non-root member once in unsigned-byte order.
/// The empty path denotes the implicit directory root.
pub trait SourceSnapshotStorage: Send + Sync {
    fn entry(&self, path: &[u8]) -> Option<SourceSnapshotEntry<'_>>;
    fn entries(&self) -> Box<dyn Iterator<Item = (&[u8], SourceSnapshotEntry<'_>)> + '_>;
}

#[derive(Clone)]
pub struct SourceSnapshot(Arc<dyn SourceSnapshotStorage>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotError {
    InvalidPath,
    NotFound,
    NotDirectory,
    NotFile,
    NotSymlink,
    InvalidRange,
}

impl SourceSnapshot {
    pub fn new(storage: impl SourceSnapshotStorage + 'static) -> Self {
        Self(Arc::new(storage))
    }

    pub fn lookup(&self, path: &[u8]) -> Result<SourceSnapshotEntry<'_>, SourceSnapshotError> {
        if !source_snapshot_path_is_canonical(path) {
            return Err(SourceSnapshotError::InvalidPath);
        }
        // Every intermediate component must be an explicit directory. A link
        // remains inert even when its target is retained in this same tree.
        for (index, byte) in path.iter().enumerate() {
            if *byte == b'/' {
                match self.0.entry(&path[..index]) {
                    Some(SourceSnapshotEntry::Directory) => {}
                    Some(_) => return Err(SourceSnapshotError::NotDirectory),
                    None => return Err(SourceSnapshotError::NotFound),
                }
            }
        }
        if path.is_empty() {
            return Ok(SourceSnapshotEntry::Directory);
        }
        self.0.entry(path).ok_or(SourceSnapshotError::NotFound)
    }

    pub fn metadata(&self, path: &[u8]) -> Result<SourceSnapshotMetadata, SourceSnapshotError> {
        Ok(match self.lookup(path)? {
            SourceSnapshotEntry::Directory => SourceSnapshotMetadata::Directory,
            SourceSnapshotEntry::File(bytes) => SourceSnapshotMetadata::File {
                byte_length: bytes.len() as u64,
            },
            SourceSnapshotEntry::Symlink(target) => SourceSnapshotMetadata::Symlink {
                target_byte_length: target.len() as u64,
            },
        })
    }

    pub fn read(&self, path: &[u8], offset: u64, count: u64) -> Result<&[u8], SourceSnapshotError> {
        let SourceSnapshotEntry::File(bytes) = self.lookup(path)? else {
            return Err(SourceSnapshotError::NotFile);
        };
        offset
            .checked_add(count)
            .ok_or(SourceSnapshotError::InvalidRange)?;
        let offset = usize::try_from(offset).map_err(|_| SourceSnapshotError::InvalidRange)?;
        if offset > bytes.len() {
            return Err(SourceSnapshotError::InvalidRange);
        }
        let count = usize::try_from(count)
            .unwrap_or(usize::MAX)
            .min(bytes.len() - offset);
        Ok(&bytes[offset..offset + count])
    }

    pub fn read_link(&self, path: &[u8]) -> Result<&[u8], SourceSnapshotError> {
        match self.lookup(path)? {
            SourceSnapshotEntry::Symlink(target) => Ok(target),
            _ => Err(SourceSnapshotError::NotSymlink),
        }
    }

    pub fn list(
        &self,
        path: &[u8],
    ) -> Result<Vec<(&[u8], SourceSnapshotEntry<'_>)>, SourceSnapshotError> {
        if self.lookup(path)? != SourceSnapshotEntry::Directory {
            return Err(SourceSnapshotError::NotDirectory);
        }
        let mut children = Vec::new();
        for (member, entry) in self.0.entries() {
            let suffix = if path.is_empty() {
                member
            } else if member.starts_with(path) && member.get(path.len()) == Some(&b'/') {
                &member[path.len() + 1..]
            } else {
                continue;
            };
            if !suffix.is_empty() && !suffix.contains(&b'/') {
                children.push((suffix, entry));
            }
        }
        children.sort_unstable_by(|left, right| left.0.cmp(right.0));
        Ok(children)
    }
}

pub fn source_snapshot_path_is_canonical(path: &[u8]) -> bool {
    std::str::from_utf8(path).is_ok()
        && !path.contains(&0)
        && !path.contains(&b'\\')
        && !(path.len() >= 2 && path[1] == b':')
        && (path.is_empty()
            || path
                .split(|byte| *byte == b'/')
                .all(|component| !component.is_empty() && component != b"." && component != b".."))
}

impl std::fmt::Debug for SourceSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceSnapshot")
            .field("entries", &self.0.entries().count())
            .finish()
    }
}

impl PartialEq for SourceSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.entries().eq(other.0.entries())
    }
}
impl Eq for SourceSnapshot {}
