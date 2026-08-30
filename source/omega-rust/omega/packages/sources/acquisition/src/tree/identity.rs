//! Canonical identity for an exact package-source tree.

use sha2::{Digest, Sha256};

use crate::SourceResolveError;
use crate::identity::digest::{format_sha256, hash_bytes, hash_length};

pub(crate) struct SourceIdentityHasher {
    hasher: Sha256,
    byte_count: u64,
}

impl SourceIdentityHasher {
    pub(crate) fn new(entry_count: usize) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"omega-source-tree-v4\0");
        hash_length(&mut hasher, entry_count as u64);
        Self {
            hasher,
            byte_count: 0,
        }
    }

    pub(crate) fn add_directory(&mut self, relative_bytes: &[u8], normalized_mode: u16) {
        self.add_path(relative_bytes);
        self.hasher.update(b"directory");
        self.hasher.update(normalized_mode.to_le_bytes());
    }

    pub(crate) fn add_file(
        &mut self,
        relative_bytes: &[u8],
        executable: bool,
        bytes: &[u8],
    ) -> Result<(), SourceResolveError> {
        self.add_path(relative_bytes);
        self.hasher.update(b"file");
        self.hasher.update([u8::from(executable)]);
        hash_bytes(&mut self.hasher, bytes);
        self.byte_count = self
            .byte_count
            .checked_add(bytes.len() as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit: u64::MAX })?;
        Ok(())
    }

    pub(crate) fn add_symlink(&mut self, relative_bytes: &[u8], target_bytes: &[u8]) {
        self.add_path(relative_bytes);
        self.hasher.update(b"symlink");
        hash_bytes(&mut self.hasher, target_bytes);
    }

    fn add_path(&mut self, relative_bytes: &[u8]) {
        self.hasher.update(b"entry");
        hash_bytes(&mut self.hasher, relative_bytes);
    }

    pub(crate) fn finish(self) -> (u64, String) {
        (self.byte_count, format_sha256(&self.hasher.finalize()))
    }
}
