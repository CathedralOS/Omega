//! The selected-transformation roster: one row per variant names the identity
//! it records, the append-only wire tag, and the rendered kind. Admission,
//! emission and rendering read this table, so a transformation cannot be
//! recorded on the wire and silently dropped from the identity or the text.

use crate::cursor::Cursor;

use super::{PostAllocationOptimizationManifestDecodeError, PostAllocationSelectedTransformation};

use selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, CopyRemovalIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, LiteralFoldIdentity, PressureRematerializationIdentity,
    RedundantCompareIdentity, RedundantExtensionIdentity, SelectedInstructionPlanIdentity,
    StoredLoadForwardingIdentity,
};

/// One row per transformation: wire tag, variant, the identity it records, and
/// the rendered kind. The tag is append-only, so a new transformation takes the
/// next unused tag and no other row moves.
macro_rules! selected_transformation_rows {
    ($( $tag:literal => $variant:ident($identity:ty) as $kind:literal; )*) => {
        impl PostAllocationSelectedTransformation {
            /// The append-only wire tag this transformation is recorded under.
            pub(super) fn wire_tag(&self) -> u8 {
                match self {
                    $( Self::$variant(_) => $tag, )*
                }
            }

            /// The identity this transformation was recorded under.
            pub(super) fn identity_bytes(&self) -> [u8; 32] {
                match self {
                    $( Self::$variant(identity) => identity.bytes(), )*
                }
            }

            /// The kind this transformation renders as.
            pub(super) fn kind(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => $kind, )*
                }
            }

            /// One wire row: the exact tag, then the 32-byte identity it seals.
            pub(super) fn decode(
                cursor: &mut Cursor<'_>,
            ) -> Result<Self, PostAllocationOptimizationManifestDecodeError> {
                Ok(match cursor.byte()? {
                    $( $tag => Self::$variant(<$identity>::from_bytes(cursor.array()?)), )*
                    tag => {
                        return Err(
                            PostAllocationOptimizationManifestDecodeError::UnknownTransformationTag(
                                tag,
                            ),
                        );
                    }
                })
            }
        }
    };
}

selected_transformation_rows! {
    1 => FixedViewCopy(FixedViewCopyIdentity) as "fixed-view-copy";
    2 => LiteralFold(LiteralFoldIdentity) as "literal-fold";
    3 => PressureRematerialization(PressureRematerializationIdentity) as "pressure-rematerialization";
    4 => RuntimeSpill(SelectedInstructionPlanIdentity) as "runtime-spill";
    5 => RuntimeRematerialization(SelectedInstructionPlanIdentity) as "runtime-rematerialization";
    6 => CopyRemoval(CopyRemovalIdentity) as "copy-removal";
    7 => RedundantExtension(RedundantExtensionIdentity) as "redundant-extension";
    8 => AddressFold(AddressFoldIdentity) as "address-fold";
    9 => ConstantBoolean(ConstantBooleanIdentity) as "constant-boolean";
    10 => ConstantBranch(ConstantBranchIdentity) as "constant-branch";
    11 => DeadCompare(DeadCompareIdentity) as "dead-compare";
    12 => RedundantCompare(RedundantCompareIdentity) as "redundant-compare";
    13 => EquivalentCompare(EquivalentCompareIdentity) as "equivalent-compare";
    14 => RuntimeSplit(SelectedInstructionPlanIdentity) as "runtime-split";
    15 => CopiedCallOperand(CopiedCallOperandIdentity) as "copied-call-operand";
    16 => StoredLoadForwarding(StoredLoadForwardingIdentity) as "stored-load-forwarding";
    17 => FixedOccurrences(SelectedInstructionPlanIdentity) as "fixed-occurrences";
}
