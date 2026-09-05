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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aarch64SameViewCopyElisionIdentity([u8; 32]);

impl Aarch64SameViewCopyElisionIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86XorZeroMaterializationIdentity([u8; 32]);

impl X86XorZeroMaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86MovR32Imm32MaterializationIdentity([u8; 32]);

impl X86MovR32Imm32MaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86MovR64Imm32SignExtendedMaterializationIdentity([u8; 32]);

impl X86MovR64Imm32SignExtendedMaterializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
