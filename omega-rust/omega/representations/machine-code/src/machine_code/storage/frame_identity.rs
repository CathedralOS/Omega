//! Content coordinates of a target frame and its encoded protocol.
//!
//! These identify replay inputs; they do not establish ABI preservation.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetFrameLayoutIdentity([u8; 32]);

impl TargetFrameLayoutIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetFrameProtocolEncodingIdentity([u8; 32]);

impl TargetFrameProtocolEncodingIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
