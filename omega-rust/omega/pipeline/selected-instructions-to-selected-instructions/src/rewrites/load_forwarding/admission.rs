//! Shared admission for store-to-load forwarding: locate the named load,
//! walk back through its block and across its predecessor edges to the
//! exact same-width write that last wrote its place range on every path,
//! and prove no intervening instruction or crossed edge can disturb it.
//!
//! `Load64` pairs with an eight-byte write of the place's storage and
//! forwards to `CopyI64`; `Load32`/`Load16`/`Load8` pair with a `Store` of
//! exactly the load's width at the identical byte offset and forward to
//! `ZeroExtendU32`/`ZeroExtendU16`/`ZeroExtendU8`: a same-width write then
//! load round-trips the stored register's low bits through the target's own
//! byte order, so the rewrite needs no endianness assumption. The writer's
//! route to the place's storage can be the referent-pointer `Store`
//! carrying `WritePlace`, or a write into the place's own local storage —
//! its `StructuralParameter` or `StructuralBlockParameter` slot — carrying
//! `WriteLocal`: a `Store` through the slot's materialized address, or a
//! `Store64` into the slot directly, which is always eight bytes and so
//! pairs only with `Load64`. A wider or shifted writer covering only part
//! of the read rejects — no selected extract can slice a register's middle
//! bytes.
//!
//! Interference is decided from the validated access roster. A row naming the
//! forwarded place blocks on any overlapping or dynamic-extent write and on
//! any materialized local address for that place; a `WriteLocal` row names
//! an exact range, so a disjoint local write — place storage or staging —
//! walks past the same way a disjoint `WritePlace` does. Rows for other
//! places and every read role are safe under place exclusivity.
//! Instructions without a row are admitted only when their kind cannot
//! write semantic storage: loads, address formation, private-slot frame
//! accesses, and pure register work. Calls, hosted effects, and unaccounted
//! writers reject.
//!
//! The walk is not confined to one block: reaching a block's top without
//! interference continues through every predecessor block, and the deferred
//! block resolves once each of those paths resolves to the same stored
//! register — a store dominating a join decides all its legs, as does each
//! leg's own store of that register. The entry block, a block no edge
//! reaches, and a deferred region whose paths never resolve to one register
//! — a self-loop or writerless cycle included — each end the walk in
//! rejection. A crossed terminator sits between its block's body and the
//! edge, so its roster rows decide first; each crossed edge is then checked
//! for transports that could redefine the carried registers or write the
//! forwarded place.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedCasePayloadTransport, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccess, SelectedMemoryAccessRole,
    SelectedStructuralTransport, SelectedSuccessor, SelectedTerminator, SelectedValueTransport,
    VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::PlaceId;

use super::StoredLoadForwardingError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub load_index: usize,
    pub load_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    pub value: VirtualRegisterId,
    pub output: VirtualRegisterId,
    /// Index of the load's single `ReadPlace` row in `memory_accesses`; replay
    /// requires the proposed roster to drop exactly this row.
    pub load_access: usize,
    pub copy: &'source RegisterInstructionConstraint,
    /// The replacement kind: `CopyI64` for a full-width forward, the matching
    /// `ZeroExtend` for an exact-width sub-word forward. Every form shares the
    /// target's plain `[use, def]` copy row.
    pub kind: SelectedInstructionKind,
}

/// One exact byte range within one place root.
struct Forwarded {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
}

impl Forwarded {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
            && u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }
}

#[test]
fn dynamic_copy_destination_blocks_forwarding_but_its_source_does_not() {
    let place = PlaceId::new(1).unwrap();
    let forwarded = Forwarded {
        place,
        byte_offset: 8,
        byte_count: 8,
    };
    let length = semantic_vocabulary::ValueId::new(3).unwrap();
    let obligation = semantic_vocabulary::ObligationId::new(4).unwrap();
    let accepted_fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([5; 32]);
    let mut access = SelectedMemoryAccess {
        instruction: SelectedInstructionId(1),
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
            semantic_vocabulary::OperationId::new(1).unwrap(),
        ),
        place,
        byte_offset: 0,
        byte_count: 0,
        role: SelectedMemoryAccessRole::WriteByteSpan {
            length,
            obligation,
            accepted_fact,
        },
    };
    assert!(interferes(&forwarded, &access));
    access.role = SelectedMemoryAccessRole::ReadByteSpan {
        length,
        obligation,
        accepted_fact,
    };
    assert!(!interferes(&forwarded, &access));
    access.role = SelectedMemoryAccessRole::WriteByteSpan {
        length,
        obligation,
        accepted_fact,
    };
    access.place = PlaceId::new(2).unwrap();
    assert!(!interferes(&forwarded, &access));
    access.place = place;
    access.role = SelectedMemoryAccessRole::WritePlace;
    assert!(
        !interferes(&forwarded, &access),
        "fixed zero-byte rows are not dynamic spans"
    );
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, StoredLoadForwardingError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(StoredLoadForwardingError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(StoredLoadForwardingError::SourceMismatch)?;
    let (block_index, load_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == load)
                .map(|load_index| (block_index, load_index))
        })
        .ok_or(StoredLoadForwardingError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let forwarded_load = &block.instructions[load_index];
    let (byte_offset, width): (u32, u32) = match forwarded_load.kind {
        SelectedInstructionKind::Load8 { byte_offset } => (byte_offset, 1),
        SelectedInstructionKind::Load16 { byte_offset } => (byte_offset, 2),
        SelectedInstructionKind::Load32 { byte_offset } => (byte_offset, 4),
        SelectedInstructionKind::Load64 { byte_offset } => (byte_offset, 8),
        _ => return Err(StoredLoadForwardingError::UnsupportedInstruction),
    };
    // The read's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction encodes.
    let mut rows = function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == load);
    let (load_access, read) = rows
        .next()
        .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
    if rows.next().is_some()
        || read.role != SelectedMemoryAccessRole::ReadPlace
        || read.byte_offset != byte_offset
        || read.byte_count != width
    {
        return Err(StoredLoadForwardingError::UnsupportedPair);
    }
    let forwarded = Forwarded {
        place: read.place,
        byte_offset: read.byte_offset,
        byte_count: width,
    };
    // The load's result must be defined only here; the copy keeps the register.
    let output = single_def(forwarded_load)?;
    let output_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == output)
        .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
    let VirtualRegisterOrigin::InstructionResult {
        instruction: output_instruction,
        ..
    } = output_register.origin
    else {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    };
    if output_instruction != load {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    // The load row must follow the target's [use pointer, def result] shape so
    // the replacement copy can reuse the result operand's class.
    let load_row = environment
        .constraint(forwarded_load.constraint)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    if load_row.operands.len() != 2
        || load_row.operands[0].operand != 0
        || load_row.operands[0].access != RegisterOperandAccess::Use
        || load_row.operands[1].operand != 1
        || load_row.operands[1].access != RegisterOperandAccess::Def
        || load_row.operands[1].class != output_register.class
    {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    // Walk back to the last writer of the forwarded range on every path to
    // the load. Within a block the first potentially interfering access
    // scanning back decides: an exact same-width write of the place's
    // storage resolves the block to the register that write carried;
    // anything else rejects. A block
    // whose body shows no interference defers to its predecessors — the walk
    // crosses every edge into it and each predecessor's terminator rows
    // decide first. A deferred block then resolves when every predecessor
    // path resolves to one register: either the same store dominates the
    // join or each leg's own last writer stored that register. The entry
    // block's implicit path, a block no edge reaches, and a deferred region
    // whose paths never agree on one register — including cycles with no
    // writer — each leave the load unproven. The carried `value` and the
    // load's `output` are validated against every crossed edge, terminator,
    // and walked span once the common register is known.
    let mut visited = vec![false; function.blocks.len()];
    let mut resolved = vec![None; function.blocks.len()];
    let mut deferred = Vec::new();
    let mut walked = Vec::new();
    let mut between = Vec::new();
    let mut crossed = Vec::new();
    let mut pending = vec![(block_index, load_index)];
    while let Some((cursor, cursor_end)) = pending.pop() {
        if visited[cursor] {
            continue;
        }
        visited[cursor] = true;
        walked.push(cursor);
        let current = &function.blocks[cursor];
        let mut found = None;
        for candidate_index in (0..cursor_end).rev() {
            let candidate = &current.instructions[candidate_index];
            reject_barrier(candidate)?;
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= interferes(&forwarded, access);
            }
            if interfered {
                found = Some(forwarding_source(
                    candidate,
                    &forwarded,
                    function,
                    environment,
                )?);
                between.push((cursor, candidate_index + 1, cursor_end));
                break;
            }
            if !has_row {
                reject_unaccounted(candidate)?;
            }
        }
        if let Some(value) = found {
            resolved[cursor] = Some(value);
            continue;
        }
        between.push((cursor, 0, cursor_end));
        // Reached the block's top. The writer must arrive on every path in:
        // the entry block has an implicit path no predecessor covers, and a
        // block no edge names leaves this path without a writer at all.
        if current.id == function.entry_block {
            return Err(StoredLoadForwardingError::UnsupportedPair);
        }
        let mut predecessors = Vec::new();
        for (predecessor_index, predecessor) in function.blocks.iter().enumerate() {
            let edges: Vec<&SelectedSuccessor> = successors(&predecessor.terminator)
                .into_iter()
                .filter(|successor| successor.block == current.id)
                .collect();
            if edges.is_empty() {
                continue;
            }
            // The terminator instruction sits between the predecessor's
            // body and the crossed edge, so an interfering row on it decides
            // first. It never has a place-storage writer kind, so it rejects
            // the pair.
            let terminator = terminator_instruction(&predecessor.terminator);
            if function
                .memory_accesses
                .iter()
                .any(|access| access.instruction == terminator.id && interferes(&forwarded, access))
            {
                return Err(StoredLoadForwardingError::AliasingWrite);
            }
            crossed.extend(edges.iter().copied());
            predecessors.push(predecessor_index);
            if !visited[predecessor_index] {
                pending.push((predecessor_index, predecessor.instructions.len()));
            }
        }
        if predecessors.is_empty() {
            return Err(StoredLoadForwardingError::UnsupportedPair);
        }
        deferred.push((cursor, predecessors));
    }
    // Resolve the deferred region: a block resolves to the one register
    // every predecessor resolved to, so convergence through a shared
    // predecessor lands while divergent legs, writerless cycles, and
    // unreached regions leave the load's block unresolved.
    let value = loop {
        if let Some(value) = resolved[block_index] {
            break value;
        }
        let mut progressed = false;
        for (block, predecessors) in &deferred {
            if resolved[*block].is_some() {
                continue;
            }
            let mut values = predecessors
                .iter()
                .map(|predecessor| resolved[*predecessor]);
            if let Some(Some(first)) = values.next()
                && values.all(|value| value == Some(first))
            {
                resolved[*block] = Some(first);
                progressed = true;
            }
        }
        if !progressed {
            return Err(StoredLoadForwardingError::UnsupportedPair);
        }
    };
    if value == output {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    let value_register = function
        .virtual_registers
        .iter()
        .find(|register| register.id == value)
        .ok_or(StoredLoadForwardingError::UnsupportedPair)?;
    // Nothing on the walked path may redefine the carried value or predefine
    // the load's result: the store's tail, each crossed block's terminator
    // and its successor-edge transports, the intervening block bodies, and
    // the load's own head all keep both registers identical after the copy.
    for successor in &crossed {
        edge_preserves(successor, &forwarded, value, output)?;
    }
    for (walked_block, start, end) in &between {
        for instruction in &function.blocks[*walked_block].instructions[*start..*end] {
            registers_untouched(instruction, value, output)?;
        }
    }
    for walked_block in &walked {
        if *walked_block != block_index {
            registers_untouched(
                terminator_instruction(&function.blocks[*walked_block].terminator),
                value,
                output,
            )?;
        }
    }
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    if copy.operands.len() != 2
        || copy.operands[0].operand != 0
        || copy.operands[0].access != RegisterOperandAccess::Use
        || copy.operands[0].class != value_register.class
        || copy.operands[1].operand != 1
        || copy.operands[1].access != RegisterOperandAccess::Def
        || copy.operands[1].class != output_register.class
        || !copy.implicit_uses.is_empty()
        || !copy.implicit_defs.is_empty()
        || !copy.clobbers.is_empty()
        || copy.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(StoredLoadForwardingError::ConstraintMismatch);
    }
    // The replacement keeps the load's exact read width: a same-width store
    // and load round-trip the register's low bits in the target's own byte
    // order, so `ZeroExtend` reconstructs the loaded value without any
    // endianness assumption. Selection already pairs these normalization
    // kinds with the target's plain copy row.
    let kind = match forwarded.byte_count {
        8 => SelectedInstructionKind::CopyI64,
        4 => SelectedInstructionKind::ZeroExtendU32,
        2 => SelectedInstructionKind::ZeroExtendU16,
        _ => SelectedInstructionKind::ZeroExtendU8,
    };
    let interval = between
        .iter()
        .try_fold(0usize, |distance, (_, start, end)| {
            distance.checked_add(end - start)
        })
        .and_then(|total| total.checked_add(crossed.len()))
        .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
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
        .ok_or(StoredLoadForwardingError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| StoredLoadForwardingError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(StoredLoadForwardingError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        load_index,
        load_id: load,
        provenance: forwarded_load.provenance.clone(),
        value,
        output,
        load_access,
        copy,
        kind,
    })
}

/// The load's sole register definition. A pinned, tied, or early-clobbered
/// result cannot become a same-identity copy through a clean copy row.
fn single_def(
    instruction: &SelectedInstruction,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let mut defs = instruction
        .operands
        .iter()
        .filter(|operand| operand.access == RegisterOperandAccess::Def);
    let result = defs
        .next()
        .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
    if defs.next().is_some()
        || result.fixed_view.is_some()
        || result.tied_to.is_some()
        || result.early_clobber
    {
        return Err(StoredLoadForwardingError::UnsupportedUse);
    }
    Ok(result.virtual_register)
}

/// Whether one roster row can disturb the forwarded bytes. Writes must target
/// the same place root to overlap; dynamic extents and escaped place-backed
/// addresses always block. A `WriteLocal` row names an exact range: whether
/// its slot is the forwarded place's storage or only stages bytes naming the
/// place, a disjoint row cannot touch the forwarded bytes — so range
/// intersection decides, and an intersecting row still has to be the exact
/// writer. Local-slot storage for a different place and outgoing-area storage
/// never alias a referent place.
fn interferes(forwarded: &Forwarded, access: &SelectedMemoryAccess) -> bool {
    match access.role {
        SelectedMemoryAccessRole::WritePlace => {
            access.place == forwarded.place && forwarded.intersects(access)
        }
        SelectedMemoryAccessRole::WriteByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. } => access.place == forwarded.place,
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            slot.structural_place() == Some(forwarded.place) && forwarded.intersects(access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            slot.structural_place() == Some(forwarded.place)
        }
        SelectedMemoryAccessRole::ReadPlace
        | SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// Whether `slot` is `place`'s own storage, so a write into it moves the
/// place's bytes in the place's byte coordinates and can source a forward:
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

/// The found writer must produce the read's exact bytes from one register:
/// one roster row on the forwarded place naming the identical byte offset
/// and byte count, on the target's own operand surface. Two routes to the
/// place's storage qualify:
/// - a `Store` of the read's exact width carrying `WritePlace` — through the
///   referent pointer — or `WriteLocal` on the place's own parameter
///   storage, through that slot's materialized address;
/// - a `Store64` into `Local(slot)` carrying `WriteLocal` on that same
///   slot — directly into the place's own parameter storage. The slot store
///   is always eight bytes, so only `Load64` pairs with it.
///
/// A wider, narrower, or shifted writer cannot produce the read's bytes from
/// one register without an extract the selected vocabulary does not carry. A
/// `WriteLocal` on an operation-owned `Structural` slot never sources: the
/// slot can stage bytes that merely name the place without being its
/// storage.
fn forwarding_source(
    instruction: &SelectedInstruction,
    forwarded: &Forwarded,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let reject = || StoredLoadForwardingError::AliasingWrite;
    // The writer's semantic identity: exactly one roster row on the
    // forwarded place naming the read's exact byte range.
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some()
        || row.place != forwarded.place
        || row.byte_offset != forwarded.byte_offset
        || row.byte_count != forwarded.byte_count
    {
        return Err(reject());
    }
    match instruction.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            if byte_offset != forwarded.byte_offset || u32::from(byte_size) != forwarded.byte_count
            {
                return Err(reject());
            }
            match row.role {
                SelectedMemoryAccessRole::WritePlace => {}
                SelectedMemoryAccessRole::WriteLocal { slot }
                    if local_slot_is_place_storage(slot, forwarded.place) => {}
                _ => return Err(reject()),
            }
            let row_constraint = environment
                .constraint(instruction.constraint)
                .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
            if row_constraint.operands.len() != 2
                || row_constraint.operands[0].operand != 0
                || row_constraint.operands[0].access != RegisterOperandAccess::Use
                || row_constraint.operands[1].operand != 1
                || row_constraint.operands[1].access != RegisterOperandAccess::Use
            {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let mut uses = instruction
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use);
            let pointer = uses.next().ok_or_else(reject)?;
            let value = uses.next().ok_or_else(reject)?;
            if uses.next().is_some() || pointer.operand != 0 || value.operand != 1 {
                return Err(reject());
            }
            Ok(value.virtual_register)
        }
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset,
        } => {
            if byte_offset != forwarded.byte_offset || forwarded.byte_count != 8 {
                return Err(reject());
            }
            // The roster row must name the same slot the instruction writes,
            // and that slot must be the forwarded place's own storage.
            if row.role != (SelectedMemoryAccessRole::WriteLocal { slot })
                || !local_slot_is_place_storage(slot, forwarded.place)
            {
                return Err(reject());
            }
            if environment.selected_keys().store64 != Some(instruction.constraint) {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let row_constraint = environment
                .constraint(instruction.constraint)
                .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
            if row_constraint.operands.len() != 1
                || row_constraint.operands[0].operand != 0
                || row_constraint.operands[0].access != RegisterOperandAccess::Use
            {
                return Err(StoredLoadForwardingError::ConstraintMismatch);
            }
            let mut uses = instruction
                .operands
                .iter()
                .filter(|operand| operand.access == RegisterOperandAccess::Use);
            let value = uses.next().ok_or_else(reject)?;
            if uses.next().is_some() || value.operand != 0 {
                return Err(reject());
            }
            Ok(value.virtual_register)
        }
        _ => Err(reject()),
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

/// The instruction a terminator positions at the end of its block. Its
/// operands and roster rows sit between the block's body and any crossed
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

/// A crossed edge must perform no work the roster cannot see. `Unused`
/// transports are quiet; `Registers` bindings and case payloads name the one
/// register the edge defines, which must not be the carried value or the
/// load's result. Structural destinations, case custody slots, and custody
/// discards reach the forwarded bytes only through the same place root under
/// place exclusivity.
fn edge_preserves(
    successor: &SelectedSuccessor,
    forwarded: &Forwarded,
    value: VirtualRegisterId,
    output: VirtualRegisterId,
) -> Result<(), StoredLoadForwardingError> {
    for binding in &successor.bindings {
        if let SelectedValueTransport::Registers { parameter, .. } = binding.transport
            && (parameter == value || parameter == output)
        {
            return Err(StoredLoadForwardingError::UnsupportedUse);
        }
    }
    for binding in &successor.structural_bindings {
        let destination = match binding.transport {
            SelectedStructuralTransport::Unused => continue,
            SelectedStructuralTransport::WholeValue { destination, .. }
            | SelectedStructuralTransport::Descriptor { destination, .. } => destination,
        };
        if destination.structural_place() == Some(forwarded.place) {
            return Err(StoredLoadForwardingError::AliasingWrite);
        }
    }
    if let Some(case) = &successor.structural_case {
        if case.slot.structural_place() == Some(forwarded.place)
            || case.trivial_affine_discards.contains(&forwarded.place)
        {
            return Err(StoredLoadForwardingError::AliasingWrite);
        }
        for payload in &case.payloads {
            let parameter = match payload.transport {
                SelectedCasePayloadTransport::Unused => continue,
                SelectedCasePayloadTransport::Unmaterialized { parameter }
                | SelectedCasePayloadTransport::Registers { parameter, .. } => parameter,
            };
            if parameter == value || parameter == output {
                return Err(StoredLoadForwardingError::UnsupportedUse);
            }
        }
    }
    Ok(())
}

/// Between the store and the load no instruction may redefine the carried
/// value or predefine the load's result; both stay register-identical after
/// the copy.
fn registers_untouched(
    instruction: &SelectedInstruction,
    value: VirtualRegisterId,
    output: VirtualRegisterId,
) -> Result<(), StoredLoadForwardingError> {
    for operand in &instruction.operands {
        if operand.access != RegisterOperandAccess::Use
            && (operand.virtual_register == value || operand.virtual_register == output)
        {
            return Err(StoredLoadForwardingError::UnsupportedUse);
        }
    }
    Ok(())
}

/// Calls, hosted effects, and terminator kinds are always barriers: they can
/// write or expose reachable storage regardless of their roster rows, and a
/// terminator kind never belongs in a block body.
fn reject_barrier(instruction: &SelectedInstruction) -> Result<(), StoredLoadForwardingError> {
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
        | ConditionalBranchI64LessThan => Err(StoredLoadForwardingError::UnsupportedInstruction),
        _ => Ok(()),
    }
}

/// An interval instruction without a roster row must be unable to write any
/// semantic or place-backed storage: loads only observe, address forms only
/// compute, private-slot frame stores touch compiler-owned spill/boundary
/// slots, and pure register work has no memory side at all. A referent store
/// or any other frame slot without its row is an unaccounted write.
fn reject_unaccounted(instruction: &SelectedInstruction) -> Result<(), StoredLoadForwardingError> {
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
        | MaterializeBooleanI64LessOrEqual => Ok(()),
        Store { .. } | StorePacked { .. } | Store64 { .. } | CopyBytes => {
            Err(StoredLoadForwardingError::AliasingWrite)
        }
        _ => Err(StoredLoadForwardingError::UnsupportedInstruction),
    }
}

/// The one-instruction proposal shape shared with replay: the target's own
/// copy row supplies the operand interface while the register identity, the
/// instruction identity, and the read's provenance stay with the result.
pub(super) fn forwarded(admitted: &Admission<'_>) -> SelectedInstruction {
    SelectedInstruction {
        id: admitted.load_id,
        kind: admitted.kind,
        constraint: admitted.copy.key,
        operands: admitted
            .copy
            .operands
            .iter()
            .zip([admitted.value, admitted.output])
            .map(
                |(operand, register)| selected_instructions::SelectedOperand {
                    operand: operand.operand,
                    virtual_register: register,
                    access: operand.access,
                    class: operand.class,
                    fixed_view: operand.fixed_view,
                    tied_to: operand.tied_to,
                    early_clobber: operand.early_clobber,
                },
            )
            .collect(),
        implicit_uses: admitted.copy.implicit_uses.clone(),
        implicit_defs: admitted.copy.implicit_defs.clone(),
        clobbers: admitted.copy.clobbers.clone(),
        provenance: admitted.provenance.clone(),
    }
}
