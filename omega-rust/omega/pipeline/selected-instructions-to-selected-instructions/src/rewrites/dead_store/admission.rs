//! Shared admission for dead-store elimination: locate the named `Store`,
//! prove its exact `WritePlace` row, then walk the same block forward to the
//! first access on the dead place and require it to be the covering store.
//!
//! Interference is decided from the validated access roster. A row naming the
//! dead place interferes when it can observe the stored bytes or leave them
//! observable: any overlapping or dynamic-extent read, any write that is not
//! the exact covering store, a place-backed local slot, or a materialized
//! local address. Rows for other places are safe under place exclusivity.
//! Instructions without a row are admitted only when their kind cannot reach
//! semantic storage: private-slot frame accesses and pure register work.
//! Calls, hosted effects, and unaccounted writers reject.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessRole,
};
use semantic_vocabulary::PlaceId;

use super::DeadStoreEliminationError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub store_index: usize,
    /// Index of the store's single `WritePlace` row in `memory_accesses`;
    /// replay requires the proposed roster to drop exactly this row.
    pub store_access: usize,
}

/// One exact byte range within one place root.
struct Dead {
    place: PlaceId,
    byte_offset: u32,
}

impl Dead {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + 8
            && u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, DeadStoreEliminationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DeadStoreEliminationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DeadStoreEliminationError::SourceMismatch)?;
    let (block_index, store_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == store)
                .map(|store_index| (block_index, store_index))
        })
        .ok_or(DeadStoreEliminationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let dead_store = &block.instructions[store_index];
    let SelectedInstructionKind::Store {
        byte_offset,
        byte_size: 8,
    } = dead_store.kind
    else {
        return Err(DeadStoreEliminationError::UnsupportedInstruction);
    };
    // The write's semantic identity: exactly one roster row, one place root,
    // and the same eight bytes the instruction encodes. `WritePlace` carries
    // no obligation payload, so dropping the row loses no proof receipt.
    let mut rows = function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == store);
    let (store_access, write) = rows
        .next()
        .ok_or(DeadStoreEliminationError::UnsupportedInstruction)?;
    if rows.next().is_some()
        || write.role != SelectedMemoryAccessRole::WritePlace
        || write.byte_offset != byte_offset
        || write.byte_count != 8
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    let dead = Dead {
        place: write.place,
        byte_offset: write.byte_offset,
    };
    // The removed instruction must be the target's plain two-use place store;
    // an exotic operand surface would make the removal contract unclear.
    place_store_shape(dead_store, environment)?;
    // Walk forward to the first access that can reach the dead bytes. It must
    // be an exact covering store of the same range; anything else leaves the
    // bytes observable or only partially overwritten.
    let mut found = None;
    for (candidate_index, candidate) in block.instructions.iter().enumerate().skip(store_index + 1)
    {
        reject_barrier(candidate)?;
        let mut has_row = false;
        let mut interfered = false;
        for access in function
            .memory_accesses
            .iter()
            .filter(|access| access.instruction == candidate.id)
        {
            has_row = true;
            interfered |= interferes(&dead, access);
        }
        if interfered {
            covering_source(candidate, &dead, function, environment)?;
            found = Some(candidate_index);
            break;
        }
        if !has_row {
            reject_unaccounted(candidate)?;
        }
    }
    let killer_index = found.ok_or(DeadStoreEliminationError::UnsupportedPair)?;
    // A boundary settlement positioned inside the dead interval is an event a
    // boundary could observe through; positions outside it only shift.
    for settlement in function
        .boundary_settlements
        .iter()
        .filter(|settlement| settlement.block == block.id)
    {
        let position = settlement.instruction_index as usize;
        if store_index < position && position <= killer_index {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(killer_index.checked_sub(store_index)?))
        .and_then(|total| total.checked_add(function.memory_accesses.len()))
        .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| DeadStoreEliminationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(DeadStoreEliminationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        store_index,
        store_access,
    })
}

/// The named store and the covering store share one operand surface: the
/// target's `[use pointer, use value]` row with nothing implicit attached.
fn place_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// Whether one roster row can observe the dead bytes or leave them
/// observable. Reads must intersect the dead range; dynamic extents always
/// reach it. Writes must target the same place root to overlap; the covering
/// store is handled by the caller after this returns true. Local-slot and
/// outgoing-area storage never aliases a referent place.
fn interferes(dead: &Dead, access: &SelectedMemoryAccess) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            access.place == dead.place && dead.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => access.place == dead.place,
        SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot.structural_place() == Some(dead.place)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// The found access must be a `Store` of all eight bytes at the identical
/// row: one `WritePlace` access, matching place and byte offset, and the
/// target's own `[pointer, value]` operand shape.
fn covering_source(
    instruction: &SelectedInstruction,
    dead: &Dead,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    let SelectedInstructionKind::Store {
        byte_offset,
        byte_size: 8,
    } = instruction.kind
    else {
        return Err(reject());
    };
    if byte_offset != dead.byte_offset {
        return Err(reject());
    }
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some()
        || row.role != SelectedMemoryAccessRole::WritePlace
        || row.place != dead.place
        || row.byte_offset != dead.byte_offset
        || row.byte_count != 8
    {
        return Err(reject());
    }
    place_store_shape(instruction, environment)
}

/// Calls, hosted effects, and terminator kinds are always barriers: they can
/// observe or expose reachable storage regardless of their roster rows, and a
/// terminator kind never belongs in a block body.
fn reject_barrier(instruction: &SelectedInstruction) -> Result<(), DeadStoreEliminationError> {
    use SelectedInstructionKind::*;
    match instruction.kind {
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
        | ConditionalBranchI64LessThan => Err(DeadStoreEliminationError::UnsupportedInstruction),
        _ => Ok(()),
    }
}

/// An interval instruction without a roster row must be unable to reach any
/// semantic or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. A referent store or any
/// other frame slot without its row is an unaccounted access.
fn reject_unaccounted(instruction: &SelectedInstruction) -> Result<(), DeadStoreEliminationError> {
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
        | ExactAddI64Immediate { .. }
        | ExactSubtractI64Immediate { .. }
        | SaturatingAddU64
        | WrappingAddI64
        | SaturatingSubtractU64
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
        | MaterializeBooleanI64LessOrEqual => Ok(()),
        Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes => {
            Err(DeadStoreEliminationError::InterveningAccess)
        }
        _ => Err(DeadStoreEliminationError::UnsupportedInstruction),
    }
}

/// The block's boundary settlements after removing the instruction at
/// `removed`: positions at or before it name instructions that stay put, and
/// every later position — including the after-body position — shifts one
/// ordinal earlier. Shared by proposal and replay so both compute the same
/// roster from the source, never from each other.
pub(super) fn shifted_boundary_settlements(
    function: &SelectedFunction,
    block: SelectedBlockId,
    removed: usize,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, DeadStoreEliminationError> {
    let body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .ok_or(DeadStoreEliminationError::SourceMismatch)?
        .instructions
        .len();
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        if settlement.block != block {
            continue;
        }
        let position = settlement.instruction_index as usize;
        if position > body {
            return Err(DeadStoreEliminationError::SourceMismatch);
        }
        if position > removed {
            settlement.instruction_index = u32::try_from(position - 1)
                .map_err(|_| DeadStoreEliminationError::IdentityOverflow)?;
        }
    }
    Ok(shifted)
}
