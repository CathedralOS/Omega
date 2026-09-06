//! Typed commitments to allocation prerequisites, not validity certificates.
//!
//! Keeping these byte identities below the analyses lets a home artifact name
//! its exact inputs without depending on the pipeline that computed them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationLegalityIdentity(pub(crate) [u8; 32]);

impl AllocationLegalityIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocatorAvailabilityIdentity(pub(crate) [u8; 32]);

impl AllocatorAvailabilityIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
