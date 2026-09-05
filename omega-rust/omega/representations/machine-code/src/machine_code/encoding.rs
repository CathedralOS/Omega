//! Content identity of layout-independent encoded instructions.
//!
//! An identity names retained bytes and rows. It does not grant encoding,
//! optimization, layout, or publication authority.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedFormEncodingIdentity([u8; 32]);

impl SelectedFormEncodingIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}
