//! Shared admission for store mutation motion: locate the named `Store`,
//! prove its exact `WritePlace` row and `[use pointer, use value]` operand
//! surface, then walk forward to the latest position that keeps the write
//! ordered before every access that could observe it.
//!
//! Sinking the store delays the write inside the window where the delay is
//! unobservable. The scan stops before the first position that must stay
//! ordered after the store: a roster row touching the moved byte range or the
//! place's dynamic storage, a call or hosted effect, a redefinition of either
//! carried register, a memory-capable instruction with no roster row, or a
//! boundary settlement. The store lands immediately before that position;
//! every kept event still observes the write in the same relative order.
//!
//! The walk is not confined to one block: reaching a block's end without a
//! stop continues through its terminator's successor edges when every edge
//! names one block, that block's only predecessor is the crossed block, and
//! the successor cannot reach any block already walked. A fork out of the
//! crossed block would drop the write from the paths that leave it; a join
//! into the successor would add the write to paths that never carried it; and
//! a successor that can return to the walked chain would close a cycle in
//! which an access before the store's original position now runs after it —
//! the walked interval only proves the forward half of that order. Each
//! crossed terminator's roster rows and register definitions decide before
//! the edge's transports, which may not redefine the carried registers or
//! write the moved place's storage. Every failed crossing lands the store at
//! the crossed block's end instead of rejecting: an earlier landing on the
//! proven path is still a real motion.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBlockId, SelectedCasePayloadTransport,
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedMemoryAccess, SelectedMemoryAccessRole, SelectedStructuralTransport, SelectedSuccessor,
    SelectedTerminator, SelectedValueTransport, VirtualRegisterId,
};
use semantic_vocabulary::PlaceId;

use super::StoreMutationMotionError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub block: SelectedBlockId,
    pub store_index: usize,
    /// The block the store lands in.
    pub target_block_index: usize,
    /// The moved store's index in the target block after the move: for a
    /// same-block move this is the stopped-before ordinal minus one (the
    /// removal shifts it left); for a crossed move it is the insertion index
    /// in the target's own vector.
    pub insert_index: usize,
}

/// One exact byte range within one place root.
struct Moved {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
}

impl Moved {
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
) -> Result<Admission<'source>, StoreMutationMotionError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(StoreMutationMotionError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(StoreMutationMotionError::SourceMismatch)?;
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
        .ok_or(StoreMutationMotionError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let moved_store = &block.instructions[store_index];
    let SelectedInstructionKind::Store {
        byte_offset,
        byte_size,
    } = moved_store.kind
    else {
        return Err(StoreMutationMotionError::UnsupportedInstruction);
    };
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(StoreMutationMotionError::UnsupportedInstruction);
    }
    // The write's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction encodes. The row names the store by
    // instruction identity, so the move retains the roster unchanged.
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == store);
    let write = rows
        .next()
        .ok_or(StoreMutationMotionError::UnsupportedInstruction)?;
    if rows.next().is_some()
        || write.role != SelectedMemoryAccessRole::WritePlace
        || write.byte_offset != byte_offset
        || write.byte_count != u32::from(byte_size)
    {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    let moved = Moved {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
    };
    // The moved instruction must be the target's plain two-use place store;
    // an exotic operand surface would make the motion contract unclear.
    place_store_shape(moved_store, environment)?;
    let (pointer, value) = carried_registers(moved_store)?;
    // Walk forward to the latest provable position. Scanning a block stops
    // before the first instruction or settlement position that must stay
    // ordered after the store; reaching a block's end cleanly crosses only a
    // unique-successor, unique-predecessor, acyclic edge.
    let mut visited = vec![false; function.blocks.len()];
    let mut interval = 0usize;
    let mut cursor = block_index;
    let mut start = store_index + 1;
    let (target_block_index, insert_index) = loop {
        visited[cursor] = true;
        let current = &function.blocks[cursor];
        let mut stop = None;
        for (index, candidate) in current.instructions.iter().enumerate().skip(start) {
            if instruction_stops(candidate, &moved, function, pointer, value)
                || settlement_before(function, current.id, index)
            {
                stop = Some(index);
                break;
            }
        }
        if let Some(index) = stop {
            interval = interval
                .checked_add(index + 1 - start)
                .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            let insert = if cursor == block_index {
                index - 1
            } else {
                index
            };
            break (cursor, insert);
        }
        interval = interval
            .checked_add(current.instructions.len() - start)
            .ok_or(StoreMutationMotionError::IdentityOverflow)?;
        // The after-body settlement slot sits between the last instruction
        // and the terminator; a settlement there stops the walk at the end.
        let end = current.instructions.len();
        if settlement_before(function, current.id, end) {
            interval = interval
                .checked_add(1)
                .ok_or(StoreMutationMotionError::IdentityOverflow)?;
            let insert = if cursor == block_index { end - 1 } else { end };
            break (cursor, insert);
        }
        // The terminator instruction sits between the body and the crossed
        // edges, so a moved-place row or a carried-register definition on it
        // lands the store at the block's end before it runs.
        let terminator = terminator_instruction(&current.terminator);
        if function
            .memory_accesses
            .iter()
            .any(|access| access.instruction == terminator.id && interferes(&moved, access))
            || defines_carried(terminator, pointer, value)
        {
            let insert = if cursor == block_index { end - 1 } else { end };
            break (cursor, insert);
        }
        let edges = successors(&current.terminator);
        let land_at_end = |insert: usize| (cursor, insert);
        let Some(first) = edges.first() else {
            // A terminator without successors leaves no later position; the
            // write still runs, just at the block's end.
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        };
        // Every path forward must reach one block: edges to distinct blocks
        // admit a path the moved write never runs on.
        if edges.iter().any(|edge| edge.block != first.block) {
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        }
        let Some(next) = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == first.block)
        else {
            return Err(StoreMutationMotionError::SourceMismatch);
        };
        // The successor must see the crossed block as its only predecessor —
        // a join adds the write to paths that never carried it — and must not
        // reach any walked block: a return path would close a cycle whose
        // unverified interval could reorder an access across the store.
        if !sole_predecessor(function, next, cursor)
            || visited[next]
            || reaches_visited(function, next, &visited)
        {
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        }
        if edges
            .iter()
            .any(|edge| edge_stops(edge, &moved, pointer, value))
        {
            break land_at_end(if cursor == block_index { end - 1 } else { end });
        }
        interval = interval
            .checked_add(edges.len())
            .and_then(|total| total.checked_add(1))
            .ok_or(StoreMutationMotionError::IdentityOverflow)?;
        cursor = next;
        start = 0;
    };
    // A move must land somewhere later: a same-block landing at the store's
    // own index means the next position was already a stop.
    if target_block_index == block_index && insert_index == store_index {
        return Err(StoreMutationMotionError::UnsupportedPair);
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
        .ok_or(StoreMutationMotionError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| StoreMutationMotionError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(StoreMutationMotionError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        block: block.id,
        store_index,
        target_block_index,
        insert_index,
    })
}

/// The moved store's operand surface: the target's `[use pointer, use value]`
/// row with nothing implicit attached.
fn place_store_shape(
    instruction: &SelectedInstruction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), StoreMutationMotionError> {
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(StoreMutationMotionError::ConstraintMismatch)?;
    if row.operands.len() != 2
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
        || row.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(StoreMutationMotionError::ConstraintMismatch);
    }
    Ok(())
}

/// The two carried registers the move keeps identical: the referent pointer
/// and the stored value. A third operand of any access would move its
/// definition or read along with the store, so the surface is exactly
/// `[operand 0 pointer, operand 1 value]` and nothing else.
fn carried_registers(
    instruction: &SelectedInstruction,
) -> Result<(VirtualRegisterId, VirtualRegisterId), StoreMutationMotionError> {
    if instruction.operands.len() != 2
        || instruction.operands[0].operand != 0
        || instruction.operands[0].access != RegisterOperandAccess::Use
        || instruction.operands[1].operand != 1
        || instruction.operands[1].access != RegisterOperandAccess::Use
        || !instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
    {
        return Err(StoreMutationMotionError::UnsupportedPair);
    }
    Ok((
        instruction.operands[0].virtual_register,
        instruction.operands[1].virtual_register,
    ))
}

/// Whether one roster row touches the moved bytes or the place's dynamic
/// storage. Exact rows must intersect the moved range; dynamic extents and
/// place-backed local slots always reach it. Outgoing-area storage never
/// aliases a referent place.
fn interferes(moved: &Moved, access: &SelectedMemoryAccess) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            access.place == moved.place && moved.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => access.place == moved.place,
        SelectedMemoryAccessRole::WriteLocal { slot }
        | SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot.structural_place() == Some(moved.place)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// Whether sliding the store past this instruction would change the observed
/// order of the write. Any of the stop conditions bound the window.
fn instruction_stops(
    candidate: &SelectedInstruction,
    moved: &Moved,
    function: &SelectedFunction,
    pointer: VirtualRegisterId,
    value: VirtualRegisterId,
) -> bool {
    let mut has_row = false;
    for access in function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == candidate.id)
    {
        has_row = true;
        if interferes(moved, access) {
            return true;
        }
    }
    is_barrier(candidate)
        || defines_carried(candidate, pointer, value)
        || (!has_row && unaccounted_kind(candidate))
}

/// Calls, hosted effects, and terminator kinds are always barriers: they can
/// observe or expose reachable storage regardless of their roster rows, and a
/// terminator kind never belongs in a block body.
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

/// An interval instruction without a roster row must be unable to reach any
/// semantic or place-backed storage: private-slot frame accesses touch
/// compiler-owned spill/boundary slots that no referent place aliases, and
/// pure register work has no memory side at all. Any other memory-capable
/// kind without a row is an unaccounted access.
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
            | WrappingAddI64
            | ExactDivideU64 { .. }
            | WrappingRemainderI64 { .. }
            | SaturatingAdd { .. }
            | SaturatingSubtract { .. }
            | SaturatingDivide { .. }
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

/// A definition of a carried register anywhere in the interval would change
/// the value the moved store reads at its new position.
fn defines_carried(
    instruction: &SelectedInstruction,
    pointer: VirtualRegisterId,
    value: VirtualRegisterId,
) -> bool {
    instruction.operands.iter().any(|operand| {
        operand.access != RegisterOperandAccess::Use
            && (operand.virtual_register == pointer || operand.virtual_register == value)
    })
}

/// A boundary settlement at a position in the crossed interval would observe
/// the place before the moved write instead of after it.
fn settlement_before(function: &SelectedFunction, block: SelectedBlockId, position: usize) -> bool {
    function.boundary_settlements.iter().any(|settlement| {
        settlement.block == block && settlement.instruction_index as usize == position
    })
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
/// rows and register definitions sit between the block's body and any crossed
/// edge.
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

/// Whether `block`'s only predecessor block is `expected`: every other
/// block's terminator is scanned for an edge naming it, so a second incoming
/// edge or a self-loop keeps the join visible.
fn sole_predecessor(function: &SelectedFunction, block: usize, expected: usize) -> bool {
    let mut predecessors = function
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            successors(&candidate.terminator)
                .iter()
                .any(|edge| edge.block == function.blocks[block].id)
                .then_some(index)
        });
    predecessors.next() == Some(expected) && predecessors.next().is_none()
}

/// Whether `start` can reach any block the walk already crossed. A return
/// path closes a cycle containing both the store's old and new positions;
/// the unverified half of that cycle could reorder a same-place access
/// across the moved write.
fn reaches_visited(function: &SelectedFunction, start: usize, visited: &[bool]) -> bool {
    let mut seen = vec![false; function.blocks.len()];
    let mut pending = vec![start];
    while let Some(index) = pending.pop() {
        if visited[index] {
            return true;
        }
        if seen[index] {
            continue;
        }
        seen[index] = true;
        for edge in successors(&function.blocks[index].terminator) {
            if let Some(target) = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == edge.block)
            {
                pending.push(target);
            }
        }
    }
    false
}

/// A crossed edge must not move the carried registers or touch the moved
/// place's storage. Register transports and case payload parameters name the
/// one register the edge defines; structural destinations, the case custody
/// slot, and custody discards write or retire place storage on the edge.
fn edge_stops(
    successor: &SelectedSuccessor,
    moved: &Moved,
    pointer: VirtualRegisterId,
    value: VirtualRegisterId,
) -> bool {
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers { parameter, .. } = binding.transport
            && (parameter == pointer || parameter == value)
        {
            return true;
        }
    }
    for binding in &successor.structural_bindings {
        let destination = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. } => destination,
        };
        if destination.structural_place() == Some(moved.place) {
            return true;
        }
    }
    if let Some(case) = &successor.structural_case {
        if case.slot.structural_place() == Some(moved.place)
            || case.trivial_affine_discards.contains(&moved.place)
        {
            return true;
        }
        for payload in &case.payloads {
            let parameter = match payload.transport {
                SelectedCasePayloadTransport::Unused => continue,
                SelectedCasePayloadTransport::Unmaterialized { parameter }
                | SelectedCasePayloadTransport::Registers { parameter, .. } => parameter,
            };
            if parameter == pointer || parameter == value {
                return true;
            }
        }
    }
    false
}

/// The function's boundary settlements after the move: positions at or after
/// the landing index in a crossed target block shift one ordinal later to
/// stay before the same instruction; every other settlement — and every
/// settlement in a same-block move — keeps its position. Shared by proposal
/// and replay so both compute the same roster from the source, never from
/// each other. A settlement inside the crossed interval cannot exist after
/// admission; finding one means the position contract was broken.
pub(super) fn shifted_boundary_settlements(
    function: &SelectedFunction,
    source_block: SelectedBlockId,
    source_index: usize,
    target_block: SelectedBlockId,
    insert_index: usize,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, StoreMutationMotionError> {
    let source_body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == source_block)
        .ok_or(StoreMutationMotionError::SourceMismatch)?
        .instructions
        .len();
    let target_body = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == target_block)
        .ok_or(StoreMutationMotionError::SourceMismatch)?
        .instructions
        .len();
    let same_block = source_block == target_block;
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        let position = settlement.instruction_index as usize;
        if settlement.block == source_block {
            if position > source_body {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
            // Positions at or before the store keep their ordinal; positions
            // inside the crossed interval cannot exist; positions past the
            // landing already name instructions that stay put.
            if position > source_index && (!same_block || position <= insert_index) {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
        } else if settlement.block == target_block {
            if position > target_body {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
            if position >= insert_index {
                settlement.instruction_index = u32::try_from(position + 1)
                    .map_err(|_| StoreMutationMotionError::IdentityOverflow)?;
            } else {
                return Err(StoreMutationMotionError::SourceMismatch);
            }
        }
    }
    Ok(shifted)
}
