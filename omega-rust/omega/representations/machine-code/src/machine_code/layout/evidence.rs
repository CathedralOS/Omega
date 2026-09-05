//! Content coordinate of an explicitly selected function-relative rewrite.
//!
//! The optimizer owns construction and replay; this identity grants neither.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct X86BranchRelaxationIdentity([u8; 32]);

impl X86BranchRelaxationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
