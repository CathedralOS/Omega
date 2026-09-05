//! Typed content coordinates for named physical rewrites, without admission.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64CbnzFusionIdentity([u8; 32]);

impl Aarch64CbnzFusionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64MovnMaterializationIdentity([u8; 32]);

impl Aarch64MovnMaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
