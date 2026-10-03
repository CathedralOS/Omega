//! Physical reuse of declared spill slots by a later victim, at storage-window
//! — interval — granularity. Reuse keeps the function's declared storage set —
//! and therefore every derived frame-byte demand — unchanged, so a slot already
//! charged to one victim is never appended a second time. It is admitted only
//! when the slot's exact access stream and the rewrite's proposed accesses
//! cannot interleave destructively.
//!
//! The victim's accesses partition into windows before any slot is considered:
//! a store to the slot serves every reload its definition can reach, so the
//! definitions that can be the last victim writer at one reload position must
//! share one home and merge into a single window, while windows joined by no
//! reload are independent intervals that need not share. A last-writer
//! fixpoint over the pending rewrite's own definition events computes each
//! reload position's reaching definitions; unioning the set each position
//! observes yields the windows. Sibling windows can never interleave with
//! each other on any slot — a foreign window's store that could be the last
//! writer at a reload would belong to that reload's window by definition — so
//! each window is colored independently against the candidates alone, and
//! windows no candidate admits share the victim's private slot harmlessly.
//!
//! The per-window check replays the candidate slot as a two-class last-writer
//! state machine over the whole function. `EXISTING` means the last observed
//! writer is one of the slot's already-admitted stores; `NEW` means it is a
//! store this window would insert. An existing load stays correct only while
//! no new store can be its last writer; a proposed reload reads its own
//! window's value only while every reaching writer is one of its stores.
//! Blocks join by union, so loops and shared successors are covered by the
//! same fixpoint — unreachable code contributes nothing because it carries no
//! real writer.
//!
//! A slot is a candidate only while every access to it follows the idiom this
//! rewrite itself emits: a zero-offset `Store64`, or a zero-offset
//! `FrameAddress` whose result feeds nothing but zero-offset `Load64`s. Any
//! other naming of the slot — a byte-oriented hosted access, a structural
//! transport destination, a case edge's storage, a nonzero offset, or an
//! address used anywhere but a load's address operand — is a use the analysis
//! cannot classify, so the slot stays private to its earlier victim.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstructionKind,
    VirtualRegisterId,
};

use super::{StorageDefinition, StoragePosition};

/// Admitted use positions for the victim, one entry per block in order. The
/// sharing check needs positions, not just block membership: a proposed reload
/// reads the slot immediately before its consumer's instruction, and a
/// block-shared reload reads once at the first unpinned use of each span —
/// an admitted span-closing instruction (a victim redefinition, or a unit
/// writer the crossing policy did not keep open) ends the span, so the next
/// unpinned use reads the slot again.
#[derive(Debug, Default)]
pub(super) struct BlockUsePositions {
    /// Instruction indices holding at least one admitted unpinned use.
    pub unpinned: Vec<usize>,
    /// Instruction indices holding at least one admitted fixed-view use.
    pub pinned: Vec<usize>,
    /// Instruction indices holding at least one use forced onto a private
    /// pair by an early-clobber tied write: its reload reads the slot at
    /// this position even when the block's shared pair serves the
    /// instruction's other operand uses.
    pub dedicated: Vec<usize>,
    /// At least one admitted use at the end-of-block position: a terminator
    /// operand or an outgoing-edge transport argument.
    pub end_of_block: bool,
}

/// The slot resolution the rewrite commits to: every emitted store and every
/// emitted reload pair names its own window's slot. `stores` runs parallel to
/// the caller's `definitions`; `loads` covers every admitted use position —
/// the mid-span positions that reuse the still-open register carry their
/// window's slot too, so consumers can resolve any use point without
/// re-deriving which positions actually emit a pair.
#[derive(Debug)]
pub(super) struct Assignment {
    /// The slot each definition's `Store64` writes.
    pub stores: Vec<LocalStorageSlotId>,
    /// Per block: reload-pair slots by source instruction index, plus the
    /// end-of-block position's slot when the block has an admitted use there.
    pub loads: Vec<BlockLoads>,
    /// `true` when at least one window fell back to the victim's private
    /// slot — the rewrite must append its declaration once, no matter how
    /// many windows share it.
    pub declares_private: bool,
    /// The victim's private slot, `Spill { register: victim }`.
    pub private: LocalStorageSlotId,
}

/// Per-block reload-pair slots.
#[derive(Debug, Default)]
pub(super) struct BlockLoads {
    /// Slot for a reload pair — real or shared — at this instruction index.
    pub instructions: std::collections::BTreeMap<usize, LocalStorageSlotId>,
    /// Slot for pairs at the end-of-block position, when the block has one.
    pub end_of_block: Option<LocalStorageSlotId>,
}

impl Assignment {
    /// The slot the rewrite's store for `definition` must name.
    pub(super) fn store_slot(&self, definition: usize) -> LocalStorageSlotId {
        self.stores[definition]
    }

    /// The slot a reload pair emitted before `index` in `block` must name —
    /// `private` for a position the use scan never admitted, which no caller
    /// reaches.
    pub(super) fn load_slot(&self, block: usize, index: usize) -> LocalStorageSlotId {
        self.loads[block]
            .instructions
            .get(&index)
            .copied()
            .unwrap_or(self.private)
    }

    /// The slot an end-of-block reload pair in `block` must name.
    pub(super) fn end_of_block_slot(&self, block: usize) -> LocalStorageSlotId {
        self.loads[block].end_of_block.unwrap_or(self.private)
    }
}

const EXISTING: u8 = 1;
const NEW: u8 = 2;

#[derive(Debug, Clone, Copy)]
enum Event {
    /// An already-admitted store writes the slot.
    ExistingStore,
    /// An already-admitted load reads the slot.
    ExistingLoad,
    /// The pending rewrite stores the window's definition after this
    /// instruction.
    NewStore,
    /// The pending rewrite reloads the victim before this instruction — or at
    /// the end-of-block position, which is ordered after every body access.
    NewLoad,
}

/// A candidate slot's classified access stream, computed once and reused
/// across every window the victim proposes against it.
struct CandidateAccesses {
    /// Per block: instruction index → the slot access it performs.
    accesses: Vec<std::collections::BTreeMap<usize, Event>>,
    /// Per block: the terminator instruction loads the slot.
    terminal_loads: Vec<bool>,
}

/// Resolve the slot every emitted store and reload names. Proposal and
/// independent replay call this one routine on their own derived inputs so
/// the resolution stays identical on both sides.
pub(super) fn assign(
    function: &SelectedFunction,
    victim: VirtualRegisterId,
    definitions: &[StorageDefinition],
    uses: &[BlockUsePositions],
    shared_reload: &[bool],
    span_closes: &[std::collections::BTreeSet<usize>],
) -> Assignment {
    let private = LocalStorageSlotId::Spill { register: victim };
    let block_count = function.blocks.len();
    // Emitted load positions: one pair ahead of each span-start, pinned, or
    // dedicated position, plus the shared end-of-block point. The mid-span
    // unpinned positions that reuse the open pair read no slot of their own.
    let mut emitted: Vec<std::collections::BTreeSet<usize>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeSet::new())
        .collect();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for index in 0..block.instructions.len() {
            if loads_before(uses, shared_reload, span_closes, block_index, index) {
                emitted[block_index].insert(index);
            }
        }
    }
    // Definitions anchored at each instruction index, in `definitions` order;
    // boundary definitions open their block ahead of every instruction.
    let mut anchored: Vec<std::collections::BTreeMap<usize, Vec<usize>>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeMap::new())
        .collect();
    let mut boundary: Vec<Vec<usize>> = vec![Vec::new(); block_count];
    for (definition, storage) in definitions.iter().enumerate() {
        match storage.position {
            StoragePosition::BlockStart => boundary[storage.block_index].push(definition),
            StoragePosition::AfterInstruction(anchor)
            | StoragePosition::AfterUseDef {
                instruction: anchor,
                ..
            } => {
                if let Some(index) = function.blocks[storage.block_index]
                    .instructions
                    .iter()
                    .position(|instruction| instruction.id == anchor)
                {
                    anchored[storage.block_index]
                        .entry(index)
                        .or_default()
                        .push(definition);
                }
            }
        }
    }
    // Reaching-definition fixpoint over the victim's own store events: the
    // state entering a block is the union of every predecessor's exit, and a
    // store makes its own definition the only last writer. The state sampled
    // at a load position is the set of stores that could have written the
    // bytes that load must see — every member of it has to land in the same
    // slot, so the set merges into one window.
    let mut entries: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); block_count];
    loop {
        let mut changed = false;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let mut state = entries[block_index].clone();
            for &definition in &boundary[block_index] {
                state.clear();
                state.insert(definition);
            }
            for index in 0..block.instructions.len() {
                if let Some(definitions_here) = anchored[block_index].get(&index) {
                    for &definition in definitions_here {
                        state.clear();
                        state.insert(definition);
                    }
                }
            }
            for successor in super::control(&block.terminator).1.into_iter().flatten() {
                if let Some(destination) = function
                    .blocks
                    .iter()
                    .position(|candidate| candidate.id == successor.block)
                {
                    let merged: std::collections::BTreeSet<usize> =
                        entries[destination].union(&state).copied().collect();
                    if merged != entries[destination] {
                        entries[destination] = merged;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Sample the reaching set at every admitted use position. A position
    // reached by no definition can arise only where dominance was already
    // refused upstream; it merges nothing and its window — store-free —
    // fails every candidate, so it resolves to the private slot.
    let mut reaching: Vec<std::collections::BTreeMap<usize, std::collections::BTreeSet<usize>>> =
        function
            .blocks
            .iter()
            .map(|_| std::collections::BTreeMap::new())
            .collect();
    let mut reaching_end: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); block_count];
    let mut parent: Vec<usize> = (0..definitions.len()).collect();
    for (block_index, block) in function.blocks.iter().enumerate() {
        let mut state = entries[block_index].clone();
        for &definition in &boundary[block_index] {
            state.clear();
            state.insert(definition);
        }
        for index in 0..block.instructions.len() {
            reaching[block_index].insert(index, state.clone());
            if let Some(definitions_here) = anchored[block_index].get(&index) {
                for &definition in definitions_here {
                    state.clear();
                    state.insert(definition);
                }
            }
        }
        reaching_end[block_index] = state;
    }
    // Every position that reads the victim merges the definitions that can
    // reach it into one window — the load can name only one slot.
    let positions = |block_index: usize| -> Vec<usize> {
        let positions = &uses[block_index];
        positions
            .unpinned
            .iter()
            .chain(&positions.pinned)
            .chain(&positions.dedicated)
            .copied()
            .collect()
    };
    fn union(parent: &mut Vec<usize>, left: usize, right: usize) {
        let left = find(parent, left);
        let right = find(parent, right);
        parent[left] = right;
    }
    let mut window_of_position: std::collections::BTreeMap<(usize, usize), Option<usize>> =
        std::collections::BTreeMap::new();
    let mut window_of_end: Vec<Option<usize>> = vec![None; block_count];
    for block_index in 0..block_count {
        for index in positions(block_index) {
            let set = &reaching[block_index][&index];
            let mut members = set.iter().copied();
            if let Some(first) = members.next() {
                for other in members {
                    union(&mut parent, first, other);
                }
                window_of_position.insert((block_index, index), Some(first));
            } else {
                window_of_position.insert((block_index, index), None);
            }
        }
        if uses[block_index].end_of_block {
            let mut members = reaching_end[block_index].iter().copied();
            if let Some(first) = members.next() {
                for other in members {
                    union(&mut parent, first, other);
                }
                window_of_end[block_index] = Some(first);
            }
        }
    }
    // Resolve the merged root for every recorded position after unioning —
    // the first member's root at visit time is not necessarily the window's.
    for mapped in window_of_position.values_mut() {
        *mapped = mapped.map(|definition| find(&mut parent, definition));
    }
    for mapped in &mut window_of_end {
        *mapped = mapped.map(|definition| find(&mut parent, definition));
    }
    // Window order is declaration order by first definition so both sides
    // bind identically; the store-free pseudo-window (empty reaching sets)
    // sorts last.
    let mut windows: Vec<std::collections::BTreeSet<usize>> = Vec::new();
    let mut window_index: std::collections::BTreeMap<usize, usize> =
        std::collections::BTreeMap::new();
    for definition in 0..definitions.len() {
        let root = find(&mut parent, definition);
        let window = *window_index.entry(root).or_insert_with(|| {
            windows.push(std::collections::BTreeSet::new());
            windows.len() - 1
        });
        windows[window].insert(definition);
    }
    let windowless = windows.len();
    windows.push(std::collections::BTreeSet::new());
    let mut assignment = Assignment {
        stores: vec![private; definitions.len()],
        loads: (0..block_count).map(|_| BlockLoads::default()).collect(),
        declares_private: false,
        private,
    };
    let mut window_slot: Vec<LocalStorageSlotId> = vec![private; windows.len()];
    // Classify each candidate once, then fit windows in order. A window takes
    // the first slot whose access stream it cannot interleave with; an
    // unclassifiable or interleaving candidate is skipped.
    let mut classified: std::collections::BTreeMap<LocalStorageSlotId, Option<CandidateAccesses>> =
        std::collections::BTreeMap::new();
    for (window, window_definitions) in windows.iter().enumerate() {
        // This window's emitted load positions.
        let mut load_marks: Vec<std::collections::BTreeSet<usize>> = function
            .blocks
            .iter()
            .map(|_| std::collections::BTreeSet::new())
            .collect();
        let mut end_marks = vec![false; block_count];
        for (&(block_index, index), &mapped) in &window_of_position {
            let member = mapped
                .map(|root| *window_index.get(&root).unwrap_or(&windowless))
                .unwrap_or(windowless);
            if member == window && emitted[block_index].contains(&index) {
                load_marks[block_index].insert(index);
            }
        }
        for (block_index, &mapped) in window_of_end.iter().enumerate() {
            if !uses[block_index].end_of_block {
                continue;
            }
            let member = mapped
                .map(|root| *window_index.get(&root).unwrap_or(&windowless))
                .unwrap_or(windowless);
            if member == window {
                end_marks[block_index] = true;
            }
        }
        // A window that stores nothing and loads nothing names no slot —
        // the store-free pseudo-window is only a mapping target for the
        // defensive empty-reaching case, where it must fall back private.
        let has_loads = load_marks.iter().any(|marks| !marks.is_empty())
            || end_marks.iter().any(|&marked| marked);
        if window_definitions.is_empty() && !has_loads {
            window_slot[window] = private;
            continue;
        }
        let mut chosen = None;
        for slot in function
            .local_storage_slots
            .iter()
            .filter(|slot| {
                slot.byte_size == 8
                    && slot.alignment == 8
                    && matches!(slot.id, LocalStorageSlotId::Spill { register } if register != victim)
            })
            .map(|slot| slot.id)
        {
            let Some(accesses) = classified
                .entry(slot)
                .or_insert_with(|| classify(function, slot))
                .as_ref()
            else {
                continue;
            };
            if shareable(
                accesses,
                function,
                definitions,
                window_definitions,
                &load_marks,
                &end_marks,
            ) {
                chosen = Some(slot);
                break;
            }
        }
        let chosen = match chosen {
            Some(slot) => slot,
            None => {
                assignment.declares_private = true;
                private
            }
        };
        window_slot[window] = chosen;
    }
    for definition in 0..definitions.len() {
        let root = find(&mut parent, definition);
        let window = *window_index.get(&root).unwrap_or(&windowless);
        assignment.stores[definition] = window_slot[window];
    }
    for (&(block_index, index), &mapped) in &window_of_position {
        let window = mapped
            .map(|root| *window_index.get(&root).unwrap_or(&windowless))
            .unwrap_or(windowless);
        assignment.loads[block_index]
            .instructions
            .insert(index, window_slot[window]);
    }
    for (block_index, &mapped) in window_of_end.iter().enumerate() {
        if uses[block_index].end_of_block {
            let window = mapped
                .map(|root| *window_index.get(&root).unwrap_or(&windowless))
                .unwrap_or(windowless);
            assignment.loads[block_index].end_of_block = Some(window_slot[window]);
        }
    }
    assignment
}

fn find(parent: &mut Vec<usize>, member: usize) -> usize {
    let mut root = member;
    while parent[root] != root {
        root = parent[root];
    }
    let mut next = member;
    while parent[next] != next {
        let up = parent[next];
        parent[next] = root;
        next = up;
    }
    root
}

/// Classify one candidate slot's existing access stream, or `None` when any
/// naming of the slot escapes the emitted idiom — the slot then stays
/// private to its earlier victim.
fn classify(function: &SelectedFunction, slot: LocalStorageSlotId) -> Option<CandidateAccesses> {
    let frame_slot = FrameStorageSlotId::Local(slot);
    // Instruction indices that are existing stores or loads of the candidate
    // slot, plus every register a zero-offset `FrameAddress` for it defines.
    let mut accesses: Vec<std::collections::BTreeMap<usize, Event>> = function
        .blocks
        .iter()
        .map(|_| std::collections::BTreeMap::new())
        .collect();
    let mut slot_addresses = std::collections::BTreeSet::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (index, instruction) in block.instructions.iter().enumerate() {
            match instruction.kind {
                SelectedInstructionKind::Store64 {
                    slot: target,
                    byte_offset,
                } if target == frame_slot => {
                    if byte_offset != 0 {
                        return None;
                    }
                    accesses[block_index].insert(index, Event::ExistingStore);
                }
                SelectedInstructionKind::FrameAddress {
                    slot: target,
                    byte_offset,
                } if target == frame_slot => {
                    if byte_offset != 0 {
                        return None;
                    }
                    slot_addresses.extend(
                        instruction
                            .operands
                            .iter()
                            .filter(|operand| operand.access != RegisterOperandAccess::Use)
                            .map(|operand| operand.virtual_register),
                    );
                }
                SelectedInstructionKind::HostedReadByte { slot: target }
                | SelectedInstructionKind::HostedWriteByteI32 { slot: target }
                    if target == slot =>
                {
                    return None;
                }
                _ => {}
            }
        }
        let terminal = super::control(&block.terminator).0;
        if names_slot(terminal, slot, frame_slot) {
            return None;
        }
        for successor in super::control(&block.terminator).1.into_iter().flatten() {
            if successor
                .structural_bindings
                .iter()
                .any(|binding| match binding.transport {
                    selected_instructions::SelectedStructuralTransport::WholeValue {
                        destination,
                        ..
                    } => destination == slot,
                    selected_instructions::SelectedStructuralTransport::Address {
                        base,
                        destination,
                        ..
                    } => {
                        destination == slot
                            || base == selected_instructions::SelectedAddressBase::Local(slot)
                    }
                    selected_instructions::SelectedStructuralTransport::Unused => false,
                })
                || successor
                    .structural_case
                    .as_ref()
                    .is_some_and(|case| case.source.local_slot() == Some(slot))
            {
                return None;
            }
        }
    }
    // Every use of a candidate-slot address must be the address operand of a
    // zero-offset `Load64`; anything else reaches the slot through a shape the
    // analysis cannot classify. A terminator load executes after every emitted
    // reload pair, so it joins the event stream at the end-of-block position.
    let mut terminal_loads = vec![false; function.blocks.len()];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (index, instruction) in block
            .instructions
            .iter()
            .enumerate()
            .chain(std::iter::once((
                usize::MAX,
                super::control(&block.terminator).0,
            )))
        {
            for operand in &instruction.operands {
                if !slot_addresses.contains(&operand.virtual_register) {
                    continue;
                }
                let legal_load = matches!(
                    instruction.kind,
                    SelectedInstructionKind::Load64 { byte_offset: 0 }
                ) && operand.access == RegisterOperandAccess::Use;
                let own_definition = matches!(instruction.kind, SelectedInstructionKind::FrameAddress { slot: target, .. }
                        if target == frame_slot)
                    && operand.access != RegisterOperandAccess::Use;
                if !legal_load && !own_definition {
                    return None;
                }
                if legal_load {
                    if index == usize::MAX {
                        terminal_loads[block_index] = true;
                    } else {
                        accesses[block_index].insert(index, Event::ExistingLoad);
                    }
                }
            }
        }
        // An address that crosses an edge could feed loads this scan never
        // sees; sharing is then unverifiable.
        for successor in super::control(&block.terminator).1.into_iter().flatten() {
            let escapes =
                successor
                    .bindings
                    .iter()
                    .any(|binding| match binding.transport {
                        selected_instructions::SelectedValueTransport::Registers {
                            argument,
                            ..
                        } => slot_addresses.contains(&argument),
                        selected_instructions::SelectedValueTransport::Unused => false,
                    }) || successor.structural_bindings.iter().any(|binding| {
                    match binding.transport {
                        selected_instructions::SelectedStructuralTransport::WholeValue {
                            argument,
                            ..
                        } => slot_addresses.contains(&argument),
                        selected_instructions::SelectedStructuralTransport::Address {
                            base: selected_instructions::SelectedAddressBase::Register(argument),
                            ..
                        } => slot_addresses.contains(&argument),
                        selected_instructions::SelectedStructuralTransport::Address {
                            base: selected_instructions::SelectedAddressBase::Local(_),
                            ..
                        }
                        | selected_instructions::SelectedStructuralTransport::Unused => false,
                    }
                }) || successor.structural_case.as_ref().is_some_and(|case| {
                    case.payloads.iter().any(|payload| {
                        matches!(
                            payload.transport,
                            selected_instructions::SelectedCasePayloadTransport::Registers {
                                argument,
                                ..
                            } if slot_addresses.contains(&argument))
                    })
                });
            if escapes {
                return None;
            }
        }
    }
    Some(CandidateAccesses {
        accesses,
        terminal_loads,
    })
}

/// Whether one window's proposed accesses can share this candidate's slot:
/// a last-writer replay where `NEW` marks only this window's stores. Sibling
/// windows bound to the same slot are deliberately absent — a sibling store
/// can never be the last victim writer at this window's reloads, or the
/// reload would have joined that window instead.
fn shareable(
    candidate: &CandidateAccesses,
    function: &SelectedFunction,
    definitions: &[StorageDefinition],
    window_definitions: &std::collections::BTreeSet<usize>,
    load_marks: &[std::collections::BTreeSet<usize>],
    end_marks: &[bool],
) -> bool {
    // Proposed positions: one store after each of the window's definitions
    // and one reload at each of its emitted load positions.
    let mut events: Vec<Vec<Event>> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| {
            let mut events = Vec::new();
            // A boundary definition's store opens the block: it is the first
            // writer event, ahead of every instruction-positioned access.
            events.extend(
                definitions
                    .iter()
                    .enumerate()
                    .filter(|(definition, storage)| {
                        window_definitions.contains(definition)
                            && storage.block_index == block_index
                            && matches!(storage.position, StoragePosition::BlockStart)
                    })
                    .map(|_| Event::NewStore),
            );
            for (index, instruction) in block.instructions.iter().enumerate() {
                if load_marks[block_index].contains(&index) {
                    events.push(Event::NewLoad);
                }
                if let Some(access) = candidate.accesses[block_index].get(&index) {
                    events.push(*access);
                }
                events.extend(
                    definitions
                        .iter()
                        .enumerate()
                        .filter(|(definition, storage)| {
                            window_definitions.contains(definition)
                                && storage.block_index == block_index
                                && match storage.position {
                                    StoragePosition::AfterInstruction(anchor)
                                    | StoragePosition::AfterUseDef {
                                        instruction: anchor,
                                        ..
                                    } => anchor == instruction.id,
                                    StoragePosition::BlockStart => false,
                                }
                        })
                        .map(|_| Event::NewStore),
                );
            }
            if end_marks[block_index] {
                events.push(Event::NewLoad);
            }
            // Emitted reload pairs sit in the block body; the terminator
            // instruction — and so an existing terminator load — runs last.
            if candidate.terminal_loads[block_index] {
                events.push(Event::ExistingLoad);
            }
            events
        })
        .collect();
    // Fixpoint on last-writer sets, then verify every load at its position.
    let mut entries = vec![0u8; function.blocks.len()];
    loop {
        let mut changed = false;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let exit = events[block_index]
                .iter()
                .fold(entries[block_index], |state, event| match event {
                    Event::ExistingStore => EXISTING,
                    Event::NewStore => NEW,
                    Event::ExistingLoad | Event::NewLoad => state,
                });
            for successor in super::control(&block.terminator).1.into_iter().flatten() {
                if let Some(destination) = function
                    .blocks
                    .iter()
                    .position(|candidate| candidate.id == successor.block)
                {
                    let merged = entries[destination] | exit;
                    if merged != entries[destination] {
                        entries[destination] = merged;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    events.iter_mut().enumerate().all(|(block_index, events)| {
        let mut state = entries[block_index];
        events.iter().all(|event| match event {
            Event::ExistingStore => {
                state = EXISTING;
                true
            }
            Event::NewStore => {
                state = NEW;
                true
            }
            Event::ExistingLoad => state & NEW == 0,
            Event::NewLoad => state == NEW,
        })
    })
}

fn names_slot(
    instruction: &selected_instructions::SelectedInstruction,
    slot: LocalStorageSlotId,
    frame_slot: FrameStorageSlotId,
) -> bool {
    match instruction.kind {
        SelectedInstructionKind::Store64 { slot: target, .. }
        | SelectedInstructionKind::FrameAddress { slot: target, .. } => target == frame_slot,
        SelectedInstructionKind::HostedReadByte { slot: target }
        | SelectedInstructionKind::HostedWriteByteI32 { slot: target } => target == slot,
        _ => false,
    }
}

fn loads_before(
    uses: &[BlockUsePositions],
    shared_reload: &[bool],
    span_closes: &[std::collections::BTreeSet<usize>],
    block_index: usize,
    index: usize,
) -> bool {
    let positions = &uses[block_index];
    if positions.pinned.contains(&index) || positions.dedicated.contains(&index) {
        return true;
    }
    if !positions.unpinned.contains(&index) {
        return false;
    }
    if !shared_reload[block_index] {
        return true;
    }
    // A shared reload loads at the first unpinned use of its span. An
    // admitted span-closing instruction — a victim redefinition, or a unit
    // writer the crossing policy did not keep open — ends the span, so the
    // first unpinned use after one opens a fresh pair and reads the slot
    // again.
    let Some(&previous) = positions
        .unpinned
        .iter()
        .rev()
        .find(|&point| *point < index)
    else {
        return true;
    };
    (previous..index).any(|point| span_closes[block_index].contains(&point))
}
