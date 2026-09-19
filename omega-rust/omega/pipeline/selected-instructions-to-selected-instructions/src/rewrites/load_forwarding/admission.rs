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
//! carrying `WritePlace`, or a write into the place's own local storage
//! carrying `WriteLocal` — its `StructuralParameter`/`StructuralBlockParameter`
//! slot, or the `Structural` slot of the operation the place's declaration
//! names as its producer: a `Store` through the slot's materialized
//! address, or a `Store64` into the slot directly, which is always eight
//! bytes and so pairs only with `Load64`. A wider or shifted writer covering only part
//! of the read rejects — no selected extract can slice a register's middle
//! bytes.
//!
//! `Load8Indexed` reads a dynamic extent instead: the single byte at its
//! `ReadByteSequence` row's payload base plus the runtime `index` the row
//! carries. Only the byte-exact writer sources the forward — a
//! `Store { 0, 1 }` through a fully computed view address whose
//! `WriteByteSequence` row names the same payload base and the same index
//! value — forwarding to `ZeroExtendU8` of the stored register. An exact
//! or local range cannot contain a runtime-placed byte, and a sequence
//! write at another offset or index may land on a different byte entirely,
//! so both still reject as the source while remaining interference. When
//! the load's own `index` resolves to a clean `MaterializeI64` — the same
//! carrier audit the dead-store covering routes run — the read byte's
//! position is fixed and `admit` collapses the extent to that one byte
//! before the walk: an exact `Store` of one byte at the resolved position
//! or a sequence write whose index lands on it then sources the forward,
//! and interference decides on the fixed position. A read index staying
//! runtime keeps the dynamic extent, but a writer whose own index resolves
//! to the same `byte_offset + index` sum still spells the read byte —
//! distinct index values do when both are constants.
//!
//! Interference is decided from the validated access roster. A row naming the
//! forwarded place blocks on any overlapping write and on
//! any materialized local address for the place's own storage. A
//! dynamic-extent write reaches only upward from its fixed offset — a span
//! covers `length` bytes there and a sequence row touches
//! `offset + index` — so it still blocks while that offset starts below the
//! read's end, and walks past once it begins at or after it. A sequence
//! write whose `index` resolves to a clean `MaterializeI64` touches exactly
//! the byte `byte_offset + index` instead, so it blocks only by landing
//! inside the read's extent — a landing anywhere off it walks past. When
//! the read
//! itself is dynamic the directions mirror the byte-sequence dead store's:
//! an exact or local row still reaches the read byte once its own extent
//! ends past the payload base, and a dynamic-extent row on the place
//! always meets it — unless the load's `index` resolved the same way,
//! collapsing the extent to that one byte before the walk. A `WriteLocal`
//! row names an exact range on a slot: when the slot is the place's own
//! storage — its parameter or block-parameter home, or the producing
//! operation's `Structural` home — an intersecting row still has to be the
//! exact writer while a disjoint one walks past like a disjoint `WritePlace`;
//! a slot that only stages bytes naming the place — a call's staged view
//! descriptor — holds none of the place's bytes at any offset, so its writes
//! and materialized address never block at all. Rows for other places and
//! every read role are safe under place exclusivity.
//! Instructions without a row are admitted only when their kind cannot
//! write semantic storage: loads, address formation, private-slot frame
//! accesses, and pure register work. Calls, hosted effects, and unaccounted
//! writers reject.
//!
//! The walk is not confined to one block: reaching a block's top without
//! interference continues through every predecessor block, and the deferred
//! block resolves once each of those paths resolves to the same stored
//! register — a store dominating a join decides all its legs, as does each
//! leg's own store of that register. A deferred block writes nothing in its
//! walked span, so a cyclic deferred region still resolves when every leg
//! arriving into it settles on one register — the region is the meet
//! fixpoint over predecessor candidates — while a self-loop or writerless
//! cycle whose arriving legs disagree, the entry block, a block no edge
//! reaches, and a region no resolved leg reaches each end the walk in
//! rejection. When the load's own block sits on such a cycle, the span
//! after the load runs between the carried register and the next
//! iteration's read, so it is verified as clear as every other walked
//! span. A crossed terminator sits between its block's body and the
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
    SelectedStructuralTransport, SelectedSuccessor, SelectedValueTransport, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::StructuralPlaceDeclaration;

use super::StoredLoadForwardingError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_instruction, terminator_successors};
use crate::rewrites::condition_state::materialized_bits;
use crate::rewrites::place_storage::{local_slot_is_place_storage, structural_place_declarations};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    pub load_index: usize,
    pub load_id: SelectedInstructionId,
    pub provenance: SelectedInstructionProvenance,
    pub value: VirtualRegisterId,
    pub output: VirtualRegisterId,
    /// Index of the load's single read row in `memory_accesses` — `ReadPlace`
    /// for a fixed-offset load, `ReadByteSequence` for the indexed byte load;
    /// replay requires the proposed roster to drop exactly this row.
    pub load_access: usize,
    pub copy: &'source RegisterInstructionConstraint,
    /// The replacement kind: `CopyI64` for a full-width forward, the matching
    /// `ZeroExtend` for an exact-width sub-word forward and for the indexed
    /// byte load's single-byte forward. Every form shares the target's plain
    /// `[use, def]` copy row.
    pub kind: SelectedInstructionKind,
}

/// The bytes the load reads within one place root: an exact range, or a
/// dynamic extent when the load is the indexed byte load — its single read
/// byte sits at `byte_offset + index` for the runtime `index` the
/// `ReadByteSequence` row carries, so every position it can touch lies at or
/// after `byte_offset` with no static upper bound. When that `index` itself
/// resolves to a clean materialized constant, `admit` collapses the extent
/// to the one byte `byte_offset + index` before the walk: the read byte's
/// position is then fixed, and every interference and source check below
/// decides on it.
struct Forwarded {
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    /// The indexed byte load's runtime index, deciding the read byte's
    /// position: `byte_offset + index`. `None` for an exact load, and for
    /// an indexed load whose index resolved — its read byte is the
    /// collapsed `byte_offset` then.
    sequence_index: Option<semantic_vocabulary::ValueId>,
}

impl Forwarded {
    /// Exact rows intersect when their half-open byte intervals share a byte;
    /// widened to u64 so edge offsets cannot wrap. A dynamic read extent is
    /// unbounded upward from `byte_offset`, so the exact row still reaches
    /// the read byte once its own extent ends past that offset — ending at
    /// or below it is the only provable disjointness.
    fn intersects(&self, access: &SelectedMemoryAccess) -> bool {
        if self.sequence_index.is_some() {
            return u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count);
        }
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
            && u64::from(self.byte_offset)
                < u64::from(access.byte_offset) + u64::from(access.byte_count)
    }

    /// A dynamic-extent row's reach is unbounded only upward: a span row
    /// covers `length` bytes starting at `byte_offset` and a sequence row
    /// touches the single byte `byte_offset + index`, so every byte the row
    /// can touch lies at or after `byte_offset`. It still reaches an exact
    /// range exactly while its fixed offset starts below the range's end; an
    /// offset at or past the end is provably disjoint however far the reach
    /// extends. A sequence row whose `index` resolves to a clean
    /// `MaterializeI64` — the same carrier audit the dead-store covering
    /// routes run — touches exactly that one byte wherever its payload base
    /// sits, so it reaches this range only by landing inside it. When the
    /// read extent is itself dynamic — its own index unresolved — an
    /// unresolved row always meets it, and a resolved landing byte meets it
    /// only at or past the payload base the read starts at.
    fn reached_by(&self, access: &SelectedMemoryAccess, function: &SelectedFunction) -> bool {
        if let SelectedMemoryAccessRole::WriteByteSequence { index, .. } = access.role
            && let Ok(landed) = constant_index(function, index)
            && let Some(position) = u64::from(access.byte_offset).checked_add(landed)
        {
            let start = u64::from(self.byte_offset);
            return if self.sequence_index.is_some() {
                position >= start
            } else {
                position >= start && position < start + u64::from(self.byte_count)
            };
        }
        if self.sequence_index.is_some() {
            return true;
        }
        u64::from(access.byte_offset) < u64::from(self.byte_offset) + u64::from(self.byte_count)
    }
}

/// A function fixture with no registers and no blocks: every sequence
/// `index` it is asked about stays unresolved, so `interferes` exercises
/// the dynamic-extent decisions alone.
#[cfg(test)]
fn bare_function() -> SelectedFunction {
    SelectedFunction {
        machine: semantic_vocabulary::MachineId::new(1).unwrap(),
        attachment: None,
        provenance: Default::default(),
        structural: None,
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: selected_instructions::SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: Vec::new(),
    }
}

#[test]
fn dynamic_copy_destination_blocks_forwarding_but_its_source_does_not() {
    let place = PlaceId::new(1).unwrap();
    let function = bare_function();
    let forwarded = Forwarded {
        place,
        byte_offset: 8,
        byte_count: 8,
        sequence_index: None,
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
    assert!(interferes(&forwarded, &access, &[], &function));
    access.role = SelectedMemoryAccessRole::ReadByteSpan {
        length,
        obligation,
        accepted_fact,
    };
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.role = SelectedMemoryAccessRole::WriteByteSpan {
        length,
        obligation,
        accepted_fact,
    };
    access.place = PlaceId::new(2).unwrap();
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.place = place;
    access.role = SelectedMemoryAccessRole::WritePlace;
    assert!(
        !interferes(&forwarded, &access, &[], &function),
        "fixed zero-byte rows are not dynamic spans"
    );
    // A dynamic write's reach is unbounded only upward from its fixed
    // offset: starting at the read's end it is provably disjoint, while
    // starting one byte earlier leaves the read's last byte reachable.
    access.role = SelectedMemoryAccessRole::WriteByteSpan {
        length,
        obligation,
        accepted_fact,
    };
    access.byte_offset = 16;
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.byte_offset = 15;
    assert!(interferes(&forwarded, &access, &[], &function));
    access.role = SelectedMemoryAccessRole::WriteByteSequence {
        index: length,
        value: length,
        length,
        obligation,
        accepted_fact,
    };
    access.byte_count = 1;
    access.byte_offset = 16;
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.byte_offset = 15;
    assert!(interferes(&forwarded, &access, &[], &function));
}

#[test]
fn dynamic_read_extent_reverses_the_interference_directions() {
    let place = PlaceId::new(1).unwrap();
    let function = bare_function();
    // The indexed byte load reads one byte at `8 + index`: unbounded upward
    // from the payload base, so an exact row reaches it only while its own
    // extent ends past 8 and a dynamic-extent row on the place always meets
    // it.
    let forwarded = Forwarded {
        place,
        byte_offset: 8,
        byte_count: 1,
        sequence_index: Some(semantic_vocabulary::ValueId::new(5).unwrap()),
    };
    let obligation = semantic_vocabulary::ObligationId::new(4).unwrap();
    let accepted_fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([5; 32]);
    let mut access = SelectedMemoryAccess {
        instruction: SelectedInstructionId(1),
        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
            semantic_vocabulary::OperationId::new(1).unwrap(),
        ),
        place,
        byte_offset: 0,
        byte_count: 9,
        role: SelectedMemoryAccessRole::WritePlace,
    };
    // An exact write ending one byte past the payload base can touch the
    // runtime byte; ending at or below it cannot.
    assert!(interferes(&forwarded, &access, &[], &function));
    access.byte_count = 8;
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.byte_count = 7;
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.byte_count = 0;
    assert!(!interferes(&forwarded, &access, &[], &function));
    // Dynamic-extent rows on the place always interfere however far below
    // their fixed offset starts.
    access.byte_count = 1;
    access.byte_offset = 0;
    access.role = SelectedMemoryAccessRole::WriteByteSequence {
        index: semantic_vocabulary::ValueId::new(6).unwrap(),
        value: semantic_vocabulary::ValueId::new(7).unwrap(),
        length: semantic_vocabulary::ValueId::new(8).unwrap(),
        obligation,
        accepted_fact,
    };
    assert!(interferes(&forwarded, &access, &[], &function));
    access.role = SelectedMemoryAccessRole::WriteByteSpan {
        length: semantic_vocabulary::ValueId::new(8).unwrap(),
        obligation,
        accepted_fact,
    };
    access.byte_count = 0;
    assert!(interferes(&forwarded, &access, &[], &function));
    // A read or a different place still walks past.
    access.role = SelectedMemoryAccessRole::ReadByteSequence {
        index: semantic_vocabulary::ValueId::new(6).unwrap(),
        length: semantic_vocabulary::ValueId::new(8).unwrap(),
        obligation,
        accepted_fact,
    };
    assert!(!interferes(&forwarded, &access, &[], &function));
    access.role = SelectedMemoryAccessRole::WriteByteSequence {
        index: semantic_vocabulary::ValueId::new(6).unwrap(),
        value: semantic_vocabulary::ValueId::new(7).unwrap(),
        length: semantic_vocabulary::ValueId::new(8).unwrap(),
        obligation,
        accepted_fact,
    };
    access.byte_count = 1;
    access.place = PlaceId::new(2).unwrap();
    assert!(!interferes(&forwarded, &access, &[], &function));
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
    let indexed = matches!(forwarded_load.kind, SelectedInstructionKind::Load8Indexed);
    let (encoded_offset, width): (u32, u32) = match forwarded_load.kind {
        SelectedInstructionKind::Load8 { byte_offset } => (byte_offset, 1),
        SelectedInstructionKind::Load16 { byte_offset } => (byte_offset, 2),
        SelectedInstructionKind::Load32 { byte_offset } => (byte_offset, 4),
        SelectedInstructionKind::Load64 { byte_offset } => (byte_offset, 8),
        // The indexed byte load encodes no offset of its own: the byte it
        // reads sits at the roster row's payload base plus the runtime
        // index the row carries.
        SelectedInstructionKind::Load8Indexed => (0, 1),
        _ => return Err(StoredLoadForwardingError::UnsupportedInstruction),
    };
    // The read's semantic identity: exactly one roster row, one place root,
    // and the same bytes the instruction reads — the encoded range on a
    // `ReadPlace` row for a fixed-offset load, or the `ReadByteSequence`
    // row's one-byte dynamic extent for the indexed byte load.
    let mut rows = function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| access.instruction == load);
    let (load_access, read) = rows
        .next()
        .ok_or(StoredLoadForwardingError::UnsupportedInstruction)?;
    if rows.next().is_some() {
        return Err(StoredLoadForwardingError::UnsupportedPair);
    }
    let sequence_index = match read.role {
        SelectedMemoryAccessRole::ReadPlace
            if !indexed && read.byte_offset == encoded_offset && read.byte_count == width =>
        {
            None
        }
        SelectedMemoryAccessRole::ReadByteSequence { index, .. }
            if indexed && read.byte_count == width =>
        {
            Some(index)
        }
        _ => return Err(StoredLoadForwardingError::UnsupportedPair),
    };
    let mut forwarded = Forwarded {
        place: read.place,
        byte_offset: read.byte_offset,
        byte_count: width,
        sequence_index,
    };
    // An indexed byte load whose own `index` resolves through the same
    // carrier audit the dead-store covering routes run — sole
    // `InstructionResult` carrier, clean `MaterializeI64` definition, no
    // edge-transport or case-payload redefinition — reads one fixed byte at
    // `byte_offset + index`: the read extent collapses to that exact byte.
    // Every check below then decides on a fixed position — a row that
    // cannot contain or land on the read byte walks past, and a write that
    // does is a candidate source — while an unresolved index, or a position
    // no u32 names, leaves the extent unbounded upward from `byte_offset`.
    if let Some(index) = forwarded.sequence_index
        && let Ok(landed) = constant_index(function, index)
        && let Some(position) = u64::from(forwarded.byte_offset).checked_add(landed)
        && let Ok(position) = u32::try_from(position)
    {
        forwarded.byte_offset = position;
        forwarded.byte_count = 1;
        forwarded.sequence_index = None;
    }
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
    // The load row must follow the target's load surface so the replacement
    // copy can reuse the result operand's class: [use pointer, def result]
    // for a fixed-offset load, [use pointer, use index, def result] for the
    // indexed byte load.
    let load_row = environment
        .constraint(forwarded_load.constraint)
        .ok_or(StoredLoadForwardingError::ConstraintMismatch)?;
    let load_shape = if indexed {
        load_row.operands.len() == 3
            && load_row.operands[0].operand == 0
            && load_row.operands[0].access == RegisterOperandAccess::Use
            && load_row.operands[1].operand == 1
            && load_row.operands[1].access == RegisterOperandAccess::Use
            && load_row.operands[2].operand == 2
            && load_row.operands[2].access == RegisterOperandAccess::Def
            && load_row.operands[2].class == output_register.class
    } else {
        load_row.operands.len() == 2
            && load_row.operands[0].operand == 0
            && load_row.operands[0].access == RegisterOperandAccess::Use
            && load_row.operands[1].operand == 1
            && load_row.operands[1].access == RegisterOperandAccess::Def
            && load_row.operands[1].class == output_register.class
    };
    if !load_shape {
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
    // join or each leg's own last writer stored that register, and a
    // deferred cycle resolves the same way once its arriving legs agree on
    // the register, since the cycle writes nothing itself. The entry
    // block's implicit path, a block no edge reaches, and a deferred region
    // whose arriving legs disagree or never settle each leave the load
    // unproven. The carried `value` and the load's `output` are validated
    // against every crossed edge, terminator, and walked span once the
    // common register is known.
    let structural_places = structural_place_declarations(function);
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
                interfered |= interferes(&forwarded, access, structural_places, function);
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
            let edges: Vec<&SelectedSuccessor> = terminator_successors(&predecessor.terminator)
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
            if function.memory_accesses.iter().any(|access| {
                access.instruction == terminator.id
                    && interferes(&forwarded, access, structural_places, function)
            }) {
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
    // Resolve the deferred region. A block resolves to the one register
    // every predecessor resolved to, so convergence through a shared
    // predecessor lands while divergent legs and unreached regions leave
    // the load's block unresolved. The first pass is acyclic propagation
    // only; a stall leaves the cyclic remainder to the fixpoint below.
    loop {
        if resolved[block_index].is_some() {
            break;
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
            break;
        }
    }
    // The still-open deferred blocks carry an optimistic candidate: nothing
    // in a deferred block's walked span writes the forwarded range, so a
    // cyclic region carries whatever its arriving legs agree on. Each open
    // block takes the meet of its predecessors — a resolved or already
    // pinned predecessor contributes its register, an open one contributes
    // nothing, and conflicting registers mark the block conflicted — and the
    // candidates only descend, so the meet converges. A block left open at
    // the fixpoint belongs to a region no resolved leg reaches: no finite
    // path arrives through it, so it constrains nothing.
    let mut open = vec![false; function.blocks.len()];
    for (block, _) in &deferred {
        open[*block] = resolved[*block].is_none();
    }
    let mut conflicted = vec![false; function.blocks.len()];
    loop {
        let mut progressed = false;
        for (block, predecessors) in &deferred {
            if !open[*block] || conflicted[*block] {
                continue;
            }
            let mut candidate = None;
            let mut conflict = false;
            for predecessor in predecessors {
                if conflicted[*predecessor] {
                    conflict = true;
                    break;
                }
                if let Some(register) = resolved[*predecessor] {
                    match candidate {
                        None => candidate = Some(register),
                        Some(current) if current == register => {}
                        Some(_) => {
                            conflict = true;
                            break;
                        }
                    }
                }
            }
            if conflict {
                conflicted[*block] = true;
                resolved[*block] = None;
                progressed = true;
            } else if resolved[*block] != candidate {
                resolved[*block] = candidate;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    let Some(value) = resolved[block_index] else {
        return Err(StoredLoadForwardingError::UnsupportedPair);
    };
    // When the load's own block sits on a deferred cycle — walked
    // predecessors reach back to it — the span after the load runs between
    // the carried register and the next iteration's read. It must be as
    // clear as every other walked span: no barrier kind, no unaccounted
    // access. An interfering write there is the last writer on the looping
    // path, so it is admitted only when it is itself a source of the carried
    // register; the register check below then treats the tail like the rest
    // of the interval.
    let head_looped = {
        let mut deferred_block = vec![false; function.blocks.len()];
        for (block, _) in &deferred {
            deferred_block[*block] = true;
        }
        let mut seen = vec![false; function.blocks.len()];
        let mut stack: Vec<usize> = deferred
            .iter()
            .find(|(block, _)| *block == block_index)
            .map(|(_, predecessors)| {
                predecessors
                    .iter()
                    .copied()
                    .filter(|predecessor| deferred_block[*predecessor])
                    .collect()
            })
            .unwrap_or_default();
        let mut loops = false;
        while let Some(block) = stack.pop() {
            if block == block_index {
                loops = true;
                break;
            }
            if seen[block] {
                continue;
            }
            seen[block] = true;
            if let Some((_, predecessors)) = deferred.iter().find(|(entry, _)| *entry == block) {
                stack.extend(
                    predecessors
                        .iter()
                        .copied()
                        .filter(|predecessor| deferred_block[*predecessor]),
                );
            }
        }
        loops
    };
    if head_looped {
        let tail = &function.blocks[block_index].instructions[load_index + 1..];
        for candidate in tail {
            reject_barrier(candidate)?;
            let mut has_row = false;
            let mut interfered = false;
            for access in function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == candidate.id)
            {
                has_row = true;
                interfered |= interferes(&forwarded, access, structural_places, function);
            }
            if interfered {
                // The interfering instruction is the last writer on the
                // looping path: the read still observes the carried register
                // only when this writer itself sourced it.
                if forwarding_source(candidate, &forwarded, function, environment)? != value {
                    return Err(StoredLoadForwardingError::AliasingWrite);
                }
                continue;
            }
            if !has_row {
                reject_unaccounted(candidate)?;
            }
        }
        between.push((
            block_index,
            load_index + 1,
            function.blocks[block_index].instructions.len(),
        ));
    }
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
    // A looped head adds its own tail span and terminator: on the cyclic
    // path they run between the load and the block's next top.
    for successor in &crossed {
        edge_preserves(successor, &forwarded, value, output)?;
    }
    for (walked_block, start, end) in &between {
        for instruction in &function.blocks[*walked_block].instructions[*start..*end] {
            registers_untouched(instruction, value, output)?;
        }
    }
    for walked_block in &walked {
        if *walked_block != block_index || head_looped {
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
/// the same place root to overlap; escaped place-backed
/// addresses always block. A dynamic-extent write on the forwarded place
/// reaches only upward from its fixed offset, so it blocks exactly while
/// that offset starts below the read's end — a write beginning at or past
/// the end is provably disjoint and walks past like a disjoint `WritePlace`,
/// as is a byte-sequence write whose resolved index lands it outside the
/// read's extent entirely, wherever its payload base sits.
/// When the read itself is a dynamic extent the directions reverse: an
/// exact-range write still reaches the runtime-placed byte once its own
/// extent ends past the payload base, while every dynamic-extent row on the
/// place meets it. A `WriteLocal` row names an exact range on a slot:
/// when the slot is the forwarded place's own storage, range intersection
/// decides and an intersecting row still has to be the exact writer; when the
/// slot only stages bytes naming the place, its bytes are not the place's at
/// any offset, so the row never blocks. A materialized local address could
/// reach the same storage by a route the roster does not bound, so it blocks
/// when its slot is the place's storage; a staged slot's address reaches only
/// the staged bytes. Local-slot storage for a different place and
/// outgoing-area storage never alias a referent place.
fn interferes(
    forwarded: &Forwarded,
    access: &SelectedMemoryAccess,
    structural_places: &[StructuralPlaceDeclaration],
    function: &SelectedFunction,
) -> bool {
    match access.role {
        SelectedMemoryAccessRole::WritePlace => {
            access.place == forwarded.place && forwarded.intersects(access)
        }
        SelectedMemoryAccessRole::WriteByteSequence { .. }
        | SelectedMemoryAccessRole::WriteByteSpan { .. } => {
            access.place == forwarded.place && forwarded.reached_by(access, function)
        }
        SelectedMemoryAccessRole::WriteLocal { slot } => {
            local_slot_is_place_storage(slot, forwarded.place, structural_places)
                && forwarded.intersects(access)
        }
        SelectedMemoryAccessRole::AddressLocal { slot } => {
            local_slot_is_place_storage(slot, forwarded.place, structural_places)
        }
        SelectedMemoryAccessRole::ReadPlace
        | SelectedMemoryAccessRole::ReadByteSpan { .. }
        | SelectedMemoryAccessRole::ReadByteSequence { .. }
        | SelectedMemoryAccessRole::WriteOutgoing { .. }
        | SelectedMemoryAccessRole::AddressOutgoing { .. } => false,
    }
}

/// The found writer must produce the read's bytes from one register: one
/// roster row on the forwarded place naming the identical byte offset and
/// byte count, on the target's own operand surface. The routes to the
/// place's storage that qualify:
/// - a `Store` of the read's exact width carrying `WritePlace` — through the
///   referent pointer — or `WriteLocal` on the place's own parameter
///   storage, through that slot's materialized address;
/// - a `Store64` into `Local(slot)` carrying `WriteLocal` on that same
///   slot — directly into the place's own parameter storage. The slot store
///   is always eight bytes, so only `Load64` pairs with it;
/// - a `Store { 0, 1 }` through a fully computed view address carrying
///   `WriteByteSequence` — the byte-exact writer for a one-byte read. Its
///   row's `byte_offset` is the payload base the `index` extends, not the
///   written position, so the route decides on the byte it lands:
///   `sequence_source` below.
///
/// A wider, narrower, or shifted writer cannot produce the read's bytes from
/// one register without an extract the selected vocabulary does not carry. A
/// `WriteLocal` on an operation-owned `Structural` slot sources only when the
/// place's declaration names that operation as the slot's producer —
/// otherwise the slot stages bytes that merely name the place, and a staging
/// row never reaches this check because it does not block.
fn forwarding_source(
    instruction: &SelectedInstruction,
    forwarded: &Forwarded,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let reject = || StoredLoadForwardingError::AliasingWrite;
    let structural_places = structural_place_declarations(function);
    // The writer's semantic identity: exactly one roster row on the
    // forwarded place naming the read's byte range — the exact range for a
    // fixed-offset read, the one fixed read byte for a resolved indexed
    // load. A `WriteByteSequence` row names a payload base instead, so its
    // route dispatches on the byte the write lands before the range check.
    let mut rows = function
        .memory_accesses
        .iter()
        .filter(|access| access.instruction == instruction.id);
    let Some(row) = rows.next() else {
        return Err(reject());
    };
    if rows.next().is_some() || row.place != forwarded.place {
        return Err(reject());
    }
    if let SelectedMemoryAccessRole::WriteByteSequence { index, .. } = row.role {
        return sequence_source(instruction, forwarded, row, index, function, environment);
    }
    if row.byte_offset != forwarded.byte_offset || row.byte_count != forwarded.byte_count {
        return Err(reject());
    }
    match instruction.kind {
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            let place_route = match (forwarded.sequence_index, row.role) {
                (None, SelectedMemoryAccessRole::WritePlace) => {
                    byte_offset == forwarded.byte_offset
                        && u32::from(byte_size) == forwarded.byte_count
                }
                (None, SelectedMemoryAccessRole::WriteLocal { slot }) => {
                    byte_offset == forwarded.byte_offset
                        && u32::from(byte_size) == forwarded.byte_count
                        && local_slot_is_place_storage(slot, forwarded.place, structural_places)
                }
                _ => false,
            };
            if !place_route {
                return Err(reject());
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
                || !local_slot_is_place_storage(slot, forwarded.place, structural_places)
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

/// The `WriteByteSequence` source route: the row claims the one byte at
/// `byte_offset + index`, so the write sources the read only when it lands
/// on the read's byte. The encoded shape is the one-byte store through a
/// fully computed view address on the plain `[use pointer, use value]` row,
/// and the sequence row's contractual one-byte count. The landing decision
/// mirrors the dead-store covering route's:
/// - a read byte whose own `index` resolved — the extent collapsed to a
///   fixed position, and a fixed-offset `Load8` reads the same fixed way —
///   is sourced exactly when the write's resolved `byte_offset + index`
///   names that one byte: a resolved index landing anywhere else writes a
///   byte the read never observes, and a runtime index lands the write
///   anywhere at or past the payload base, so it can never provably spell
///   the one fixed byte;
/// - a read byte still placed at runtime is sourced by equal payload base
///   and equal `index` value — the same position at runtime — or by
///   distinct index values when both resolve to constants whose
///   `byte_offset + index` sums agree. Anything else may land on a
///   different byte entirely.
///
/// The operand audit is the `Store` route's own: the target's two-use place
/// store row, and the stored register at operand 1 is the forwarded value.
fn sequence_source(
    instruction: &SelectedInstruction,
    forwarded: &Forwarded,
    row: &SelectedMemoryAccess,
    written: semantic_vocabulary::ValueId,
    function: &SelectedFunction,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<VirtualRegisterId, StoredLoadForwardingError> {
    let reject = || StoredLoadForwardingError::AliasingWrite;
    if !matches!(
        instruction.kind,
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1
        }
    ) || row.byte_count != 1
    {
        return Err(reject());
    }
    let lands_on_read = match forwarded.sequence_index {
        None => {
            forwarded.byte_count == 1
                && constant_index(function, written)
                    .ok()
                    .and_then(|landed| u64::from(row.byte_offset).checked_add(landed))
                    == Some(u64::from(forwarded.byte_offset))
        }
        Some(index) if written == index => row.byte_offset == forwarded.byte_offset,
        Some(index) => match (
            constant_index(function, written),
            constant_index(function, index),
        ) {
            (Ok(written), Ok(read)) => {
                u64::from(row.byte_offset).checked_add(written)
                    == u64::from(forwarded.byte_offset).checked_add(read)
            }
            _ => false,
        },
    };
    if !lands_on_read {
        return Err(reject());
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

/// The compile-time constant a byte-sequence row's `index` resolves to, when
/// it does — the same carrier audit the dead-store covering routes run. The
/// register carrying the `index` value is its sole `InstructionResult`
/// carrier, so an `index` no instruction result carries (an entry or block
/// parameter) has no producer to resolve, and two instruction results
/// claiming one value make the constant ambiguous; both stay unproven. The
/// carrier must then hold the function's one clean `MaterializeI64`
/// definition and never be redefined by an edge transport or case payload
/// the instruction audit cannot see — only then does `byte_offset + index`
/// name a fixed position rather than a runtime-placed byte.
fn constant_index(
    function: &SelectedFunction,
    index: semantic_vocabulary::ValueId,
) -> Result<u64, StoredLoadForwardingError> {
    let reject = || StoredLoadForwardingError::AliasingWrite;
    let mut carriers = function.virtual_registers.iter().filter(|register| {
        matches!(
            register.origin,
            VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == index
        )
    });
    let carrier = carriers.next().ok_or_else(reject)?;
    if carriers.next().is_some() {
        return Err(reject());
    }
    let landed = materialized_bits(function, carrier.id).map_err(|_| reject())?;
    if transport_defines(function, carrier.id) {
        return Err(reject());
    }
    Ok(landed)
}

/// Whether an edge transport or case payload defines `register` — a
/// definition the instruction-operand audit in `materialized_bits` cannot
/// see, which would falsify the constant it reports for the index.
fn transport_defines(function: &SelectedFunction, register: VirtualRegisterId) -> bool {
    for block in &function.blocks {
        for successor in terminator_successors(&block.terminator) {
            if successor.bindings.iter().any(|binding| {
                matches!(
                    binding.transport,
                    SelectedValueTransport::Registers { parameter, .. } if parameter == register
                )
            }) {
                return true;
            }
            if let Some(case) = &successor.structural_case
                && case.payloads.iter().any(|payload| {
                    matches!(
                        payload.transport,
                        SelectedCasePayloadTransport::Unmaterialized { parameter }
                            | SelectedCasePayloadTransport::Registers { parameter, .. }
                            if parameter == register
                    )
                })
            {
                return true;
            }
        }
    }
    false
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
        | ExactMultiplyI64 { .. }
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
