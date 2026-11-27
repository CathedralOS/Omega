//! Non-following lookup and reads from the input retained for this Build activation.

use super::{CapturedBuildSourceInput, CapturedSourceEntry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotEntry<'storage> {
    Directory,
    File(&'storage [u8]),
    Symlink(&'storage [u8]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotMetadata {
    Directory,
    File { byte_length: u64 },
    Symlink { target_byte_length: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSnapshotError {
    InvalidPath,
    NotFound,
    NotDirectory,
    SymlinkTraversal,
    NotFile,
    NotSymlink,
    InvalidRange,
}

fn snapshot_entry(entry: &CapturedSourceEntry) -> SourceSnapshotEntry<'_> {
    match entry {
        CapturedSourceEntry::Directory => SourceSnapshotEntry::Directory,
        CapturedSourceEntry::File { bytes, .. } => SourceSnapshotEntry::File(bytes),
        CapturedSourceEntry::Symlink { target, .. } => SourceSnapshotEntry::Symlink(target),
    }
}

impl CapturedBuildSourceInput {
    pub fn lookup(&self, path: &[u8]) -> Result<SourceSnapshotEntry<'_>, SourceSnapshotError> {
        self.entry_at(path).map(snapshot_entry)
    }

    fn entry_at(&self, path: &[u8]) -> Result<&CapturedSourceEntry, SourceSnapshotError> {
        if !filesystem_authority::source_snapshot_path_is_canonical(path, true) {
            return Err(SourceSnapshotError::InvalidPath);
        }
        for (index, byte) in path.iter().enumerate() {
            if *byte == b'/' {
                match self.retained_entry(&path[..index]).map(snapshot_entry) {
                    Some(SourceSnapshotEntry::Directory) => {}
                    Some(SourceSnapshotEntry::Symlink(_)) => {
                        return Err(SourceSnapshotError::SymlinkTraversal);
                    }
                    Some(SourceSnapshotEntry::File(_)) => {
                        return Err(SourceSnapshotError::NotDirectory);
                    }
                    None => return Err(SourceSnapshotError::NotFound),
                }
            }
        }
        if path.is_empty() {
            return Ok(&CapturedSourceEntry::Directory);
        }
        self.retained_entry(path)
            .ok_or(SourceSnapshotError::NotFound)
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

    /// Open the retained backing so independent readers can share its bytes.
    pub fn open_file(&self, path: &[u8]) -> Result<&std::sync::Arc<[u8]>, SourceSnapshotError> {
        match self.entry_at(path)? {
            CapturedSourceEntry::File { bytes, .. } => Ok(bytes),
            CapturedSourceEntry::Directory => Err(SourceSnapshotError::NotFile),
            CapturedSourceEntry::Symlink { .. } => Err(SourceSnapshotError::SymlinkTraversal),
        }
    }

    pub fn read(&self, path: &[u8], offset: u64, count: u64) -> Result<&[u8], SourceSnapshotError> {
        let bytes = self.open_file(path)?;
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
        for (member, entry) in self.retained_entries() {
            let suffix = if path.is_empty() {
                member
            } else if member.starts_with(path) && member.get(path.len()) == Some(&b'/') {
                &member[path.len() + 1..]
            } else {
                continue;
            };
            if !suffix.is_empty() && !suffix.contains(&b'/') {
                children.push((suffix, snapshot_entry(entry)));
            }
        }
        Ok(children)
    }
}
