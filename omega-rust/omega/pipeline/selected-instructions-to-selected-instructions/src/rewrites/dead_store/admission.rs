//! Shared admission for dead-store elimination: locate the named `Store`,
//! prove its exact `WritePlace` row, then walk forward to the first access on
//! the dead place and require it to be a place store whose own row covers the
//! dead range entirely.
//!
//! Interference is decided from the validated access roster. A row naming the
//! dead place interferes when it can observe the stored bytes or leave them
//! observable: any overlapping or dynamic-extent read, any write that is not
//! the exact covering store, a place-backed local slot, or a materialized
//! local address. Rows for other places are safe under place exclusivity.
//! Instructions without a row are admitted only when their kind cannot reach
//! semantic storage: private-slot frame accesses and pure register work.
//! Calls, hosted effects, and unaccounted writers reject.
//!
//! The walk is not confined to one block: reaching a block's end without
//! interference continues through its terminator's successor edges when every
//! edge names one block, since each path forward from the dead store then
//! reaches that block — a join there is harmless because coverage is
//! forward-looking. The terminator's roster rows decide first; each crossed
//! edge's transports may not write or retire the dead place's storage. A
//! terminator with no successors, edges fanning out to distinct blocks, and
//! re-entering a walked block each leave an uncovered path, so they end the
//! walk in rejection.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralTransport, SelectedSuccessor, SelectedTerminator,
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
    byte_count: u32,
}

impl Dead {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
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
        byte_size,
    } = dead_store.kind
    else {
        return Err(DeadStoreEliminationError::UnsupportedInstruction);
    };
    // The dead store is the target's plain exact-width place store; packed
    // fragment widths select `StorePacked`, whose extra scratch operand would
    // leave its defining register orphaned by removal.
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(DeadStoreEliminationError::UnsupportedInstruction);
    }
    // The write's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction encodes. `WritePlace` carries
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
        || write.byte_count != u32::from(byte_size)
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    let dead = Dead {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
    };
    // The removed instruction must be the target's plain two-use place store;
    // an exotic operand surface would make the removal contract unclear.
    place_store_shape(dead_store, environment)?;
    // Walk forward to the first access that can reach the dead bytes. It must
    // be a place store whose row covers the dead range entirely; anything else
    // leaves the bytes observable or only partially overwritten. Reaching a
    // block's end without interference crosses into its only successor block —
    // every edge out naming one block means each path forward from the store
    // arrives there — checking the terminator's roster rows and each crossed
    // edge's transports on the way.
    let mut visited = vec![false; function.blocks.len()];
    let mut crossed = Vec::new();
    let mut interval = 0usize;
    let mut cursor = block_index;
    let mut cursor_start = store_index + 1;
    let (killer_block, killer_index) = loop {
        visited[cursor] = true;
        let current = &function.blocks[cursor];
        let mut found = None;
        for (candidate_index, candidate) in
            current.instructions.iter().enumerate().skip(cursor_start)
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
        if let Some(candidate_index) = found {
            interval = interval
                .checked_add(candidate_index + 1 - cursor_start)
                .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
            break (cursor, candidate_index);
        }
        interval = interval
            .checked_add(current.instructions.len() - cursor_start)
            .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
        // The terminator instruction sits between the body and the crossed
        // edges, so its roster rows decide first; a terminator kind never
        // carries the covering store.
        let terminator = terminator_instruction(&current.terminator);
        if function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == terminator.id && interferes(&dead, access))
        {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
        // Every path forward must reach one block: a terminator with no
        // successors lets the bytes escape to the boundary, and edges to
        // distinct blocks admit a path the covering store never runs on.
        let edges = successors(&current.terminator);
        let Some(first) = edges.first() else {
            return Err(DeadStoreEliminationError::UnsupportedPair);
        };
        if edges.iter().any(|edge| edge.block != first.block) {
            return Err(DeadStoreEliminationError::UnsupportedPair);
        }
        for edge in &edges {
            edge_unobserved(edge, &dead)?;
        }
        interval = interval
            .checked_add(edges.len())
            .ok_or(DeadStoreEliminationError::IdentityOverflow)?;
        crossed.push(current.id);
        let next = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == first.block)
            .ok_or(DeadStoreEliminationError::SourceMismatch)?;
        // Re-entering a walked block closes a cycle that never covers.
        if visited[next] {
            return Err(DeadStoreEliminationError::UnsupportedPair);
        }
        cursor = next;
        cursor_start = 0;
    };
    // A boundary settlement positioned inside the dead interval is an event a
    // boundary could observe through; positions outside it only shift. The
    // interval covers the store's block after the removed store, every fully
    // crossed block, and the covering block through the covering store.
    for settlement in function.boundary_settlements.iter() {
        let position = settlement.instruction_index as usize;
        if settlement.block == block.id {
            if store_index < position && (killer_block != block_index || position <= killer_index) {
                return Err(DeadStoreEliminationError::InterveningAccess);
            }
        } else if crossed.contains(&settlement.block)
            || (killer_block != block_index
                && settlement.block == function.blocks[killer_block].id
                && position <= killer_index)
        {
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
        .and_then(|total| total.checked_add(interval))
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
/// write is checked by the caller after this returns true. Local-slot and
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

/// The found access must be a place store whose single `WritePlace` row
/// covers the dead range entirely: a `Store` of any exact width or a packed
/// `StorePacked`, each encoding the same byte range its row names. A write
/// that only partially overlaps the dead range leaves the remaining bytes
/// observable, and a place-backed local slot or materialized local address
/// writes or exposes different storage — neither can cover.
fn covering_source(
    instruction: &SelectedInstruction,
    dead: &Dead,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    // The encoded byte range must equal the row's exact range: a `Store`
    // writes its `byte_size` at `byte_offset` through the referent pointer,
    // and a `StorePacked` writes its packed `width` the same way.
    let (encoded_offset, encoded_size) = match instruction.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(reject());
            }
            place_store_shape(instruction, environment)?;
            (byte_offset, u32::from(byte_size))
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            if environment.selected_keys().store_packed != Some(instruction.constraint) {
                return Err(DeadStoreEliminationError::ConstraintMismatch);
            }
            (byte_offset, u32::from(width.byte_size()))
        }
        _ => return Err(reject()),
    };
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
        || row.byte_offset != encoded_offset
        || row.byte_count != encoded_size
    {
        return Err(reject());
    }
    // Coverage is containment, not equality: the killer's bytes may start
    // before and extend past the dead range, but every dead byte must be
    // inside the row the killer writes.
    if row.byte_offset > dead.byte_offset
        || u64::from(row.byte_offset) + u64::from(row.byte_count)
            < u64::from(dead.byte_offset) + u64::from(dead.byte_count)
    {
        return Err(reject());
    }
    Ok(())
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

/// The successor edges a terminator can take: a jump's single edge or a
/// conditional's two legs. Returns and hosted exits have none.
fn successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Return { .. } | SelectedTerminator::HostedExitProcess { .. } => {
            Vec::new()
        }
    }
}

/// The instruction a terminator positions at the end of its block. Its roster
/// rows sit between the block's body and any crossed edge.
fn terminator_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

/// A crossed edge must not touch the dead place's storage. Register
/// transports cannot reach memory, but a structural destination, the case
/// custody slot, or a custody discard naming the dead place writes or retires
/// its bytes inside the dead interval.
fn edge_unobserved(
    successor: &SelectedSuccessor,
    dead: &Dead,
) -> Result<(), DeadStoreEliminationError> {
    for binding in &successor.structural_bindings {
        let destination = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. } => destination,
        };
        if destination.structural_place() == Some(dead.place) {
            return Err(DeadStoreEliminationError::InterveningAccess);
        }
    }
    if let Some(case) = &successor.structural_case
        && (case.slot.structural_place() == Some(dead.place)
            || case.trivial_affine_discards.contains(&dead.place))
    {
        return Err(DeadStoreEliminationError::InterveningAccess);
    }
    Ok(())
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
