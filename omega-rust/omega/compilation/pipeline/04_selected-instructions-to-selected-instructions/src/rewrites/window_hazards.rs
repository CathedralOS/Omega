//! Hazard audit shared by the in-block relocation rewrites: which
//! instructions may trade order at all, which pairs are coupled by a
//! register or condition-state hazard, and whether a boundary settlement
//! sits inside a window. Each rewrite locates its own window; the audit of
//! that window is one owner.
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionKind,
};
/// What an instruction without a roster row can reach. Private work is
/// register-only or touches a compiler-owned spill/boundary slot that no
/// referent place aliases. A store or byte copy outside that set could write
/// rostered storage. Every other memory-capable kind is unaccounted: no
/// rewrite can prove its reach.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::rewrites) enum UnrosteredReach {
    Private,
    Store,
    Unaccounted,
}

pub(in crate::rewrites) fn unrostered_reach(instruction: &SelectedInstruction) -> UnrosteredReach {
    use SelectedInstructionKind::*;
    match instruction.kind {
        Store64 {
            slot:
                FrameStorageSlotId::Local(
                    LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
                ),
            ..
        }
        | Load8 { .. }
        | Load16 { .. }
        | Load32 { .. }
        | Load64 { .. }
        | Load8Indexed
        | LoadPacked { .. }
        | FrameAddress { .. }
        | AddressOffset { .. }
        | ByteViewAddress
        | CopyI64
        | MaterializeI64 { .. }
        | CompareI64
        | CompareI64Zero
        | CompareI64Immediate { .. }
        | ExactAddI64 { .. }
        | ExactSubtractI64 { .. }
        | ExactMultiplyI64 { .. }
        | ExactAddI64Immediate { .. }
        | ExactSubtractI64Immediate { .. }
        | SaturatingAdd { .. }
        | WrappingAddI64
        | WrappingSubtractI64
        | WrappingMultiplyI64
        | SaturatingSubtract { .. }
        | SaturatingDivide { .. }
        | SaturatingRemainder { .. }
        | SaturatingMultiply { .. }
        | ExactDivideU64 { .. }
        | ExactRemainderU64 { .. }
        | WrappingRemainderI64 { .. }
        | WrappingDivideI64 { .. }
        | ExactDivideI64 { .. }
        | ExactRemainderI64 { .. }
        | BitwiseAndI64
        | BitwiseOrI64
        | BitwiseNotI64
        | BitwiseXorI64
        | ZeroExtendU8
        | ZeroExtendU16
        | ZeroExtendU32
        | SignExtendI8
        | SignExtendI16
        | SignExtendI32
        | Float32ToBits
        | Float64ToBits
        | BitsToFloat32
        | BitsToFloat64
        | MaterializeBooleanEqual
        | MaterializeBooleanU64LessThan
        | MaterializeBooleanI64LessThan
        | MaterializeBooleanU64LessOrEqual
        | MaterializeBooleanI64LessOrEqual => UnrosteredReach::Private,
        Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes | ZeroBytes => {
            UnrosteredReach::Store
        }
        _ => UnrosteredReach::Unaccounted,
    }
}
