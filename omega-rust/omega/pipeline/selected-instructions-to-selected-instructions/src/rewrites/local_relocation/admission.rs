//! Shared admission for in-block relocation: locate the named member
//! inside one block's body, locate the named destination instruction at a
//! different position in the same block, and prove the window they bound
//! independent — no register or condition-state hazard between the member
//! and any crossed instruction, no roster-carrying member sharing the
//! window with a second memory-access actor, no call, hosted-effect, or
//! terminator barrier anywhere in the window, and no boundary settlement
//! inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, VirtualRegisterId,
};

use super::LocalRelocationError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    /// The member's index in the block body.
    pub member_index: usize,
    /// The index whose instruction the member displaces: the window the
    /// relocation crosses is `member_index..=destination_index` in either
    /// order, a single adjacent step at distance one.
    pub destination_index: usize,
}

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

fn register_reads(
    instruction: &SelectedInstruction,
) -> impl Iterator<Item = VirtualRegisterId> + '_ {
    instruction
        .operands
        .iter()
        .filter(|operand| reads(operand.access))
        .map(|operand| operand.virtual_register)
}

fn register_writes(
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

/// Whether the member can trade order with one crossed instruction. The
/// three hazards cover every crossed position the move passes: `member`
/// defining a location `crossed` reads (RAW) would starve the consumer or
/// hand it a different value once the member lands on the other side,
/// `member` reading a location `crossed` writes (WAR) would hand the member
/// the new value, and a shared written location (WAW) would change which
/// definition later positions observe. Units and registers participate
/// identically. Hazards between two crossed instructions cannot change:
/// the crossed run keeps its relative order.
fn coupled(member: &SelectedInstruction, crossed: &SelectedInstruction) -> bool {
    writes_meet_reads(member, crossed)
        || writes_meet_reads(crossed, member)
        || register_writes(member)
            .any(|register| register_writes(crossed).any(|candidate| candidate == register))
        || member
            .implicit_defs
            .iter()
            .chain(member.clobbers.iter())
            .any(|unit| crossed.implicit_defs.contains(unit) || crossed.clobbers.contains(unit))
}

/// Calls, hosted effects, and terminator kinds are always barriers: they
/// can observe or expose reachable state regardless of roster rows, and a
/// terminator kind never belongs in a block body. Same boundary the
/// memory-motion rules enforce.
fn is_barrier(instruction: &SelectedInstruction) -> bool {
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
            | ExactAddI64Immediate { .. }
            | ExactSubtractI64Immediate { .. }
            | SaturatingAdd { .. }
            | WrappingAddI64
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
            | ExactDivideU64 { .. }
            | WrappingRemainderI64 { .. }
            | BitwiseAndI64
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
fn has_memory_rows(function: &SelectedFunction, instruction: SelectedInstructionId) -> bool {
    function
        .memory_accesses
        .iter()
        .any(|access| access.instruction == instruction)
}

/// A call contract row makes the instruction an effect barrier even when
/// its kind survived the kind check.
fn has_call_contract(function: &SelectedFunction, instruction: SelectedInstructionId) -> bool {
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
fn interior_settlement(
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
fn schedulable(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
) -> Result<bool, LocalRelocationError> {
    if is_barrier(instruction) || has_call_contract(function, instruction.id) {
        return Err(LocalRelocationError::UnsupportedInstruction);
    }
    let accounted = has_memory_rows(function, instruction.id);
    if !accounted && unaccounted_kind(instruction) {
        return Err(LocalRelocationError::UnsupportedInstruction);
    }
    Ok(accounted)
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, LocalRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LocalRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LocalRelocationError::SourceMismatch)?;
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(LocalRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The window is the span between the member and the destination in
    // this block; every other instruction inside it belongs to the crossed
    // run the member passes. The destination at the member's own position
    // names no move.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| *position != member_index)
        .ok_or(LocalRelocationError::UnsupportedPair)?;
    let member_instruction = &block.instructions[member_index];
    let member_accounted = schedulable(function, member_instruction)?;
    let (first, last) = (
        member_index.min(destination_index),
        member_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    for (offset, crossed) in window.iter().enumerate() {
        if first + offset == member_index {
            continue;
        }
        // Every crossed instruction meets the same schedulable bar as the
        // member: no barrier kind, no call contract, and no unaccounted
        // memory reach. Its roster rows may keep their relative order only
        // while the member records none — a row-carrying member passing a
        // second accounted actor would reorder recorded accesses.
        let crossed_accounted = schedulable(function, crossed)?;
        if member_accounted && crossed_accounted {
            return Err(LocalRelocationError::UnsupportedPair);
        }
        // The member trades order with every crossed position, so each
        // direction of every hazard applies against each.
        if coupled(member_instruction, crossed) {
            return Err(LocalRelocationError::UnsupportedPair);
        }
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(LocalRelocationError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every crossed member's operand and unit lists
    // plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window.iter().try_fold(total, |total, instruction| {
                total
                    .checked_add(instruction.operands.len())?
                    .checked_add(instruction.implicit_uses.len())?
                    .checked_add(instruction.implicit_defs.len())?
                    .checked_add(instruction.clobbers.len())
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(LocalRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LocalRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LocalRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        destination_index,
    })
}
