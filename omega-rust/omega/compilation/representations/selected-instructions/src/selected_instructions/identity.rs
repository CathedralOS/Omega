#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualRegisterId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedBlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SelectedInstructionId(pub u32);

/// A canonical-bytes identity newtype: `Sha256` over the owner's canonical,
/// relocation-independent encoding. The declarative row is the type's whole
/// definition — every member shares the identical three-method surface.
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

sha256_identity!(SelectedInstructionPlanIdentity);
sha256_identity!(FixedViewCopyIdentity);
sha256_identity!(PressureRematerializationIdentity);
sha256_identity!(CopyRemovalIdentity);
sha256_identity!(CopiedCallOperandIdentity);
sha256_identity!(RedundantExtensionIdentity);
sha256_identity!(AddressFoldIdentity);
sha256_identity!(ConstantBooleanIdentity);
sha256_identity!(ConstantBranchIdentity);
sha256_identity!(DeadCompareIdentity);
sha256_identity!(RedundantCompareIdentity);
sha256_identity!(EquivalentCompareIdentity);
sha256_identity!(LiteralFoldIdentity);
sha256_identity!(StoredLoadForwardingIdentity);
