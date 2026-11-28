/*
  Dense integer identifiers (`VirtualRegisterId`, `SelectedBlockId`,
  `SelectedInstructionId`) and the SHA-256 content-identity newtype macro the
  machine-effect catalog identity uses.
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualRegisterId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedBlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedInstructionId(pub u32);

macro_rules! sha256_identity {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn from_canonical_bytes(bytes: &[u8]) -> Self {
                Self(crate::digest_encoder::content_identity(&bytes).into())
            }

            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

pub(crate) use sha256_identity;
