//! Hazard audit shared by the in-block relocation rewrites: which
//! instructions may trade order at all, which pairs are coupled by a
//! register or condition-state hazard, and whether a boundary settlement
//! sits inside a window. Each rewrite locates its own window; the audit of
//! that window is one owner.
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
};

/// Register locations an operand reads or writes. `UseDef` participates in
/// both directions, matching the access marks the constraint rows publish.
fn reads(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

fn writes(operand_access: RegisterOperandAccess) -> bool {
    matches!(
        operand_access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}

pub(super) fn register_reads(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| reads(operand.access))
        .map(|operand| operand.virtual_register)
}

pub(super) fn register_writes(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| writes(operand.access))
        .map(|operand| operand.virtual_register)
}

/// Whether `writer`'s writes meet `reader`'s reads on a register or a
/// condition-state unit. Implicit uses read units; implicit definitions and
/// clobbers write them, so flag publishers and flag consumers couple the
/// same way explicit operands do.
fn writes_meet_reads(writer: &SelectedInstruction, reader: &SelectedInstruction) -> bool {
    register_writes(writer)
        .any(|register| register_reads(reader).any(|candidate| candidate == register))
        || writer
            .implicit_defs
            .iter()
            .chain(writer.clobbers.iter())
            .any(|unit| reader.implicit_uses.contains(unit))
}

/// Whether two instructions can trade order. The three hazards cover every
/// pair a move passes: `earlier` defining a location `later` reads (RAW)
/// would starve the consumer or hand it a different value, `earlier`
/// reading a location `later` writes (WAR) would hand it the new value, and
/// a shared written location (WAW) would change which definition later
/// positions observe. Units and registers participate identically.
pub(super) fn coupled(earlier: &SelectedInstruction, later: &SelectedInstruction) -> bool {
    writes_meet_reads(earlier, later)
        || writes_meet_reads(later, earlier)
        || register_writes(earlier)
            .any(|register| register_writes(later).any(|candidate| candidate == register))
        || earlier
            .implicit_defs
            .iter()
            .chain(earlier.clobbers.iter())
            .any(|unit| later.implicit_defs.contains(unit) || later.clobbers.contains(unit))
}

/// Calls, hosted effects, and terminator kinds are always barriers: they
/// can observe or expose reachable state regardless of roster rows, and a
/// terminator kind never belongs in a block body. Same boundary the
/// memory-motion rules enforce.
pub(super) fn is_barrier(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    matches!(
        instruction.kind,
        CallUnit { .. }
            | CallScalar { .. }
            | CallAggregate { .. }
            | HostedReadByte { .. }
            | HostedWriteByteI32 { .. }
            | HostedExitProcessI32
            | ReturnScalar
            | ReturnAggregate { .. }
            | ReturnUnit
            | Jump
            | ConditionalBranchNonZero
            | ConditionalBranchU64LessThan
            | ConditionalBranchI64LessThan
    )
}

/// An instruction without a roster row must be unable to reach any semantic
/// or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. Any other memory-capable
/// kind without a row is an unaccounted access whose reach the relocation
/// cannot prove.
fn unaccounted_kind(instruction: &SelectedInstruction) -> bool {
    use SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        Store64 {
            slot: FrameStorageSlotId::Local(
                LocalStorageSlotId::Spill { .. } | LocalStorageSlotId::Boundary { .. },
            ),
            ..
        } | Load8 { .. }
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
            | ExactDivideU64 { .. }
            | ExactRemainderU64 { .. }
            | WrappingRemainderI64 { .. }
            | WrappingDivideI64 { .. }
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
            | MaterializeBooleanI64LessOrEqual
    )
}

/// Whether the roster accounts for the instruction's memory reach. Rows
/// name the instruction by identity, so the relocation retains them
/// unchanged.
pub(super) fn has_memory_rows(
    function: &SelectedFunction,
    instruction: SelectedInstructionId,
) -> bool {
    function
        .memory_accesses
        .iter()
        .any(|access| access.instruction == instruction)
}

/// A call contract row makes the instruction an effect barrier even when
/// its kind survived the kind check.
pub(super) fn has_call_contract(
    function: &SelectedFunction,
    instruction: SelectedInstructionId,
) -> bool {
    function
        .calls
        .iter()
        .any(|call| call.instruction == instruction)
}

/// A boundary settlement at `position` sits before that body ordinal: any
/// position after the window's first index through its last observes a
/// different executed prefix once the member lands on the other side of
/// the crossed run, while positions at or outside the window's span see
/// the same executed set on either order.
pub(super) fn interior_settlement(
    function: &SelectedFunction,
    block: SelectedBlockId,
    window: std::ops::RangeInclusive<usize>,
) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && window.contains(&(settlement.instruction_index as usize))
    })
}

/// Whether one instruction may trade order with a crossed instruction at
/// all: not a barrier kind, not named by the call roster, and either
/// roster-accounted or unable to reach storage the roster covers.
/// `None` when the instruction can never trade order; otherwise whether the
/// roster accounts for its memory reach.
pub(super) fn schedulable(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
) -> Option<bool> {
    if is_barrier(instruction) || has_call_contract(function, instruction.id) {
        return None;
    }
    let accounted = has_memory_rows(function, instruction.id);
    if !accounted && unaccounted_kind(instruction) {
        return None;
    }
    Some(accounted)
}

/// One instruction's hazard-audit surface: the operand, implicit-use,
/// implicit-definition, and clobber rows `coupled` walks.
pub(super) fn surface(instruction: &SelectedInstruction) -> usize {
    instruction.operands.len()
        + instruction.implicit_uses.len()
        + instruction.implicit_defs.len()
        + instruction.clobbers.len()
}
