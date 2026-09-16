//! Shared admission for dead-store elimination: locate the named `Store` or
//! `StorePacked`, prove its exact `WritePlace` row, then walk forward to the
//! first access on the dead place and require it to be a write of the dead
//! place's storage whose own row covers the dead range entirely.
//!
//! Interference is decided from the validated access roster. A row naming the
//! dead place interferes when it can observe the stored bytes or leave them
//! observable: any overlapping or dynamic-extent read, any write that is not
//! the exact covering write, or a materialized local address. Rows for other
//! places are safe under place exclusivity. A `WriteLocal` on the dead
//! place's own storage — its `StructuralParameter` or
//! `StructuralBlockParameter` slot — interferes exactly like a `WritePlace`
//! on that place: an overlapping row decides coverage below, a disjoint row
//! walks past. A `Structural` operation slot can instead stage bytes that
//! merely name the place (a call's staged view descriptor), so any write to
//! it stays a barrier rather than a route to the place's storage.
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
    // The dead store is one of the target's place stores: the plain
    // exact-width `Store`, or the `StorePacked` an odd fragment width
    // selects. The packed form's extra early-clobber scratch `Def` drops
    // with the instruction, so its admission additionally proves that
    // register occurs nowhere else in the function — a surviving mention
    // would lose its definition to the removal.
    let (encoded_offset, encoded_size, packed) = match dead_store.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(DeadStoreEliminationError::UnsupportedInstruction);
            }
            (byte_offset, u32::from(byte_size), false)
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            (byte_offset, u32::from(width.byte_size()), true)
        }
        _ => return Err(DeadStoreEliminationError::UnsupportedInstruction),
    };
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
        || write.byte_offset != encoded_offset
        || write.byte_count != encoded_size
    {
        return Err(DeadStoreEliminationError::UnsupportedPair);
    }
    let dead = Dead {
        place: write.place,
        byte_offset: write.byte_offset,
        byte_count: write.byte_count,
    };
    // The removed instruction must keep the operand surface its kind
    // declares: the plain two-use place store, or the packed store's
    // two-use-plus-dead-scratch row. An exotic operand surface would make
    // the removal contract unclear.
    if packed {
        packed_store_shape(dead_store, function, environment)?;
    } else {
        place_store_shape(dead_store, environment)?;
    }
    // Walk forward to the first access that can reach the dead bytes. It must
    // be a write of the dead place's storage whose row covers the dead range
    // entirely; anything else leaves the bytes observable or only partially
    // overwritten. Reaching a block's end without interference crosses into
    // its only successor block — every edge out naming one block means each
    // path forward from the store arrives there — checking the terminator's
    // roster rows and each crossed edge's transports on the way.
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

/// The removed packed store carries the target's declared `store_packed`
/// row: `[use pointer, use packed value]` plus the early-clobber scratch
/// `Def` the multi-instruction store writes through. Removing the
/// instruction removes that definition, so the scratch register must occur
/// nowhere else in the function — any other operand position, terminator
/// operand, or successor transport still naming it would observe a
/// definition the rewrite stopped making.
fn packed_store_shape(
    instruction: &SelectedInstruction,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    if environment.selected_keys().store_packed != Some(instruction.constraint) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    let row = environment
        .constraint(instruction.constraint)
        .ok_or(DeadStoreEliminationError::ConstraintMismatch)?;
    if row.operands.len() != 3
        || row.operands[0].operand != 0
        || row.operands[0].access != RegisterOperandAccess::Use
        || row.operands[1].operand != 1
        || row.operands[1].access != RegisterOperandAccess::Use
        || row.operands[2].operand != 2
        || row.operands[2].access != RegisterOperandAccess::Def
        || !row.operands[2].early_clobber
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    if instruction.operands.len() != 3
        || instruction.operands[2].operand != 2
        || instruction.operands[2].access != RegisterOperandAccess::Def
    {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    if !scratch_definition_is_dead(function, instruction.operands[2].virtual_register) {
        return Err(DeadStoreEliminationError::ConstraintMismatch);
    }
    Ok(())
}

/// Whether `register`'s only occurrence in `function` is one `Def` operand —
/// the custody the packed store's dropped scratch requires. The operand
/// itself is that one occurrence: any other operand position, terminator
/// operand, or successor transport naming the register would leave the
/// elimination removing a definition a surviving read or second definition
/// still observes.
fn scratch_definition_is_dead(
    function: &SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        for successor in successors(&block.terminator) {
            occurrences += successor
                .structural_bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        SelectedStructuralTransport::WholeValue { argument, .. }
                            | SelectedStructuralTransport::Descriptor { argument, .. }
                            if argument == register
                    )
                })
                .count();
            if let Some(case) = &successor.structural_case {
                occurrences += case
                    .payloads
                    .iter()
                    .filter(|payload| match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter,
                        } => *parameter == register,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => *argument == register || *parameter == register,
                    })
                    .count();
            }
            occurrences += successor
                .bindings
                .iter()
                .filter(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

/// Whether `slot` is `place`'s own storage, so a write into it moves the
/// place's bytes in the place's byte coordinates and can cover a dead write:
/// a parameter home or a block parameter. An operation-owned `Structural`
/// slot can instead stage bytes that merely name the place — a call's
/// staged view descriptor — so it is not the place's storage here.
fn local_slot_is_place_storage(slot: LocalStorageSlotId, place: PlaceId) -> bool {
    match slot {
        LocalStorageSlotId::StructuralParameter { place: slot_place }
        | LocalStorageSlotId::StructuralBlockParameter {
            place: slot_place, ..
        } => slot_place == place,
        LocalStorageSlotId::Spill { .. }
        | LocalStorageSlotId::Structural { .. }
        | LocalStorageSlotId::Boundary { .. } => false,
    }
}

/// Whether one roster row can observe the dead bytes or leave them
/// observable. Reads must intersect the dead range; dynamic extents always
/// reach it. Writes must target the same place root to overlap; the covering
/// write is checked by the caller after this returns true. A `WriteLocal`
/// row names an exact range: whether its slot is the dead place's storage or
/// only stages bytes naming the place, a disjoint row cannot touch the dead
/// bytes — so range intersection decides, and an intersecting row still has
/// to cover. A materialized local address could reach the same storage by a
/// route the roster does not bound, so it always interferes. Outgoing-area
/// storage never aliases a referent place.
fn interferes(dead: &Dead, access: &SelectedMemoryAccess) -> bool {
    match access.role {
        SelectedMemoryAccessRole::ReadPlace | SelectedMemoryAccessRole::WritePlace => {
            access.place == dead.place && dead.intersects(access)
        }
        SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. }
        | SelectedMemoryAccessRole::WriteByteSequence { .. } => access.place == dead.place,
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            slot.structural_place() == Some(dead.place) && dead.intersects(access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot.structural_place() == Some(dead.place)
        }
        SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// The found access must be a write of the dead place's storage whose single
/// roster row covers the dead range entirely and names the same bytes the
/// instruction encodes:
/// - `Store` of any exact width or packed `StorePacked` carrying `WritePlace`
///   — through a place pointer — or `WriteLocal` on the place's own
///   parameter storage, through that slot's materialized address;
/// - `Store64` into `Local(slot)` carrying `WriteLocal` on that same slot —
///   directly into the place's own parameter storage.
///
/// A write that only partially overlaps the dead range leaves the remaining
/// bytes observable. A `WriteLocal` on an operation-owned `Structural` slot
/// never covers: the slot can stage bytes that merely name the place — a
/// call's staged view descriptor — without being its storage. A
/// materialized local address exposes storage rather than writing it.
fn covering_source(
    instruction: &SelectedInstruction,
    dead: &Dead,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<(), DeadStoreEliminationError> {
    let reject = || DeadStoreEliminationError::InterveningAccess;
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some() || row.place != dead.place {
        return Err(reject());
    }
    // The encoded byte range must equal the row's exact range, and the row's
    // role must match the route the instruction takes to the dead place's
    // storage.
    let (encoded_offset, encoded_size) = match instruction.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if !matches!(byte_size, 1 | 2 | 4 | 8) {
                return Err(reject());
            }
            place_store_shape(instruction, environment)?;
            match row.role {
                SelectedMemoryAccessRole::WritePlace => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if local_slot_is_place_storage(slot, dead.place) => {}
                _ => return Err(reject()),
            }
            (byte_offset, u32::from(byte_size))
        }
        SelectedInstructionKind::StorePacked { byte_offset, width } => {
            if environment.selected_keys().store_packed != Some(instruction.constraint) {
                return Err(DeadStoreEliminationError::ConstraintMismatch);
            }
            match row.role {
                SelectedMemoryAccessRole::WritePlace => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if local_slot_is_place_storage(slot, dead.place) => {}
                _ => return Err(reject()),
            }
            (byte_offset, u32::from(width.byte_size()))
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => {
            if environment.selected_keys().store64 != Some(instruction.constraint) {
                return Err(DeadStoreEliminationError::ConstraintMismatch);
            }
            // The direct slot store covers only when the roster names the
            // same slot and that slot is the dead place's own storage.
            if row.role != (SelectedMemoryAccessRole::WriteLocal { slot })
                || !local_slot_is_place_storage(slot, dead.place)
            {
                return Err(reject());
            }
            (byte_offset, 8)
        }
        _ => return Err(reject()),
    };
    if row.byte_offset != encoded_offset || row.byte_count != encoded_size {
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
        | SaturatingAddI32
        | SaturatingSubtractI32
        | SaturatingDivideI32 { .. }
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
