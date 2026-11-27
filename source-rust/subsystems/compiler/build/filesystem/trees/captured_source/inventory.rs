//! The size and canonical metadata identity of an immutable Build source snapshot.
use super::CapturedBuildSourceInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCanonicalSourceMetadataIdentity {
    policy_version: u32,
    source_content_commitment: [u8; 32],
}

impl BuildCanonicalSourceMetadataIdentity {
    #[doc(hidden)]
    pub const fn new(policy_version: u32, source_content_commitment: [u8; 32]) -> Self {
        Self {
            policy_version,
            source_content_commitment,
        }
    }

    pub const fn policy_version(self) -> u32 {
        self.policy_version
    }

    pub const fn source_content_commitment(self) -> [u8; 32] {
        self.source_content_commitment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCapturedSourceInventory {
    entry_count: u64,
    file_bytes: u64,
    source_metadata_identity: BuildCanonicalSourceMetadataIdentity,
}

impl BuildCapturedSourceInventory {
    pub const fn source_metadata_identity(self) -> BuildCanonicalSourceMetadataIdentity {
        self.source_metadata_identity
    }

    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    pub const fn file_bytes(self) -> u64 {
        self.file_bytes
    }
}

impl CapturedBuildSourceInput {
    pub fn inventory(&self) -> BuildCapturedSourceInventory {
        BuildCapturedSourceInventory {
            entry_count: self.entry_count(),
            file_bytes: self.file_bytes(),
            source_metadata_identity: BuildCanonicalSourceMetadataIdentity::new(
                self.canonical_source_metadata().policy_version(),
                *self.canonical_source_metadata().source_content_commitment(),
            ),
        }
    }
}
