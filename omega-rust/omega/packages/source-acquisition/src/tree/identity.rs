//! Canonical identity for an exact package-source tree.

use sha2::{Digest, Sha256};

use crate::SourceResolveError;
use crate::identity::digest::{format_sha256, hash_bytes, hash_length};

pub(crate) struct SourceIdentityHasher {
    hasher: crate::digest_encoder::DomainDigestEncoder,
    byte_count: u64,
}

impl SourceIdentityHasher {
    pub(crate) fn new(entry_count: usize) -> Self {
        let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"omega-source-tree-v4\0");
        hasher.u64(entry_count as u64);
        Self {
            hasher,
            byte_count: 0,
        }
    }

    pub(crate) fn add_directory(&mut self, relative_bytes: &[u8], normalized_mode: u16) {
        self.add_path(relative_bytes);
        self.hasher.raw(b"directory");
        self.hasher.raw(&normalized_mode.to_le_bytes());
    }

    pub(crate) fn add_file(
        &mut self,
        relative_bytes: &[u8],
        executable: bool,
        bytes: &[u8],
    ) -> Result<(), SourceResolveError> {
        self.add_path(relative_bytes);
        self.hasher.raw(b"file");
        self.hasher.byte(u8::from(executable));
        self.hasher.bytes(bytes);
        self.byte_count = self
            .byte_count
            .checked_add(bytes.len() as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit: u64::MAX })?;
        Ok(())
    }

    pub(crate) fn add_symlink(&mut self, relative_bytes: &[u8], target_bytes: &[u8]) {
        self.add_path(relative_bytes);
        self.hasher.raw(b"symlink");
        self.hasher.bytes(target_bytes);
    }

    fn add_path(&mut self, relative_bytes: &[u8]) {
        self.hasher.raw(b"entry");
        self.hasher.bytes(relative_bytes);
    }

    pub(crate) fn finish(self) -> (u64, String) {
        (self.byte_count, format_sha256(&self.hasher.finish()))
    }
}
