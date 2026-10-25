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

use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterOperandAccess};
use selected_instructions::{
    LocalStorageSlotId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, VirtualRegister, VirtualRegisterId,
};
use semantic_vocabulary::{IeeeFloatFormat, ScalarType};

use super::{BitsConversion, RuntimeSpillError, StorageDefinition, StoragePosition};
mod reaching;

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

impl BlockUsePositions {
    /// Record one admitted operand use in the audited ordering: a fixed-view
    /// use lands in the pinned list, any other use flags `flexible` and lands
    /// in the unpinned list, and a consecutive duplicate position is recorded
    /// once.
    pub(crate) fn record_use(
        &mut self,
        instruction_index: usize,
        fixed_view: bool,
        flexible: &mut bool,
    ) {
        let recorded = if fixed_view {
            &mut self.pinned
        } else {
            *flexible = true;
            &mut self.unpinned
        };
        if recorded.last() != Some(&instruction_index) {
            recorded.push(instruction_index);
        }
    }
}

/// The block holding an instruction id: a victim's producing-instruction
/// origins resolve to the block that instruction lands in.
pub(super) fn instruction_block(
    function: &SelectedFunction,
    instruction: SelectedInstructionId,
) -> Option<usize> {
    function
        .blocks
        .instruction_block(instruction)
        .or_else(|| {
            function.blocks.iter().position(|block| {
                super::control(&block.terminator).0.id == instruction
            })
        })
}

/// The slot resolution the rewrite commits to: every emitted store and every
/// emitted reload pair names its own window's slot. `stores` runs parallel to
/// the caller's `definitions`; `loads` covers every admitted use position —
/// the mid-span positions that reuse the still-open register carry their
/// window's slot too, so consumers can resolve any use point without
/// re-deriving which positions actually emit a pair.
#[derive(Debug)]
pub(crate) struct Assignment {
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
pub(crate) struct BlockLoads {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StorePosition {
    Boundary,
    Instruction(usize),
}

/// A candidate slot's classified access stream, computed once and reused
/// across every window the victim proposes against it.
struct CandidateAccesses {
    /// Per block: sorted `(instruction index, event)` pairs.
    accesses: Vec<Vec<(usize, Event)>>,
    /// Per block: the terminator instruction loads the slot.
    terminal_loads: Vec<bool>,
    /// Per block: the last `ExistingStore` instruction index, if any.
    last_existing_store: Vec<Option<usize>>,
    /// Per block: whether the block contains any `ExistingLoad` (body or terminator).
    has_loads: Vec<bool>,
}

struct WindowSummary {
    boundary_store: Vec<bool>,
    anchored_stores: Vec<Vec<usize>>,
    last_store: Vec<Option<StorePosition>>,
    has_loads: Vec<bool>,
}

struct ShareableScratch {
    exits: Vec<Option<u8>>,
    entries: Vec<u8>,
    queued: Vec<bool>,
    pending: std::collections::VecDeque<usize>,
}

impl ShareableScratch {
    fn new(block_count: usize) -> Self {
        Self {
            exits: vec![None; block_count],
            entries: vec![0; block_count],
            queued: vec![false; block_count],
            pending: std::collections::VecDeque::with_capacity(block_count),
        }
    }
}

/// Resolve the slot every emitted store and reload names. Proposal and
/// independent replay call this one routine on their own derived inputs so
/// the resolution stays identical on both sides. Without `reuse_slots` (the
/// pass budget declined the search) no declared slot is a candidate, so every
/// window resolves to the victim's private slot.
pub(super) fn assign(
    function: &SelectedFunction,
    victim: VirtualRegisterId,
    definitions: &[StorageDefinition],
    uses: &[BlockUsePositions],
    shared_reload: &[bool],
    span_closes: &[std::collections::BTreeSet<usize>],
    reuse_slots: bool,
) -> Assignment {
    let private = LocalStorageSlotId::Spill { register: victim };
    let block_count = function.blocks.len();
    let mut candidate_slots: Vec<LocalStorageSlotId> = Vec::new();
    if reuse_slots {
        for slot in &function.local_storage_slots {
            if slot.byte_size == 8
                && slot.alignment == 8
                && matches!(slot.id, LocalStorageSlotId::Spill { register } if register != victim)
                && !candidate_slots.contains(&slot.id)
            {
                candidate_slots.push(slot.id);
            }
        }
    }
    if candidate_slots.is_empty() {
        let mut loads: Vec<BlockLoads> = (0..block_count).map(|_| BlockLoads::default()).collect();
        let mut any_emitted_load = false;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let positions = &uses[block_index];
            if positions.end_of_block {
                loads[block_index].end_of_block = Some(private);
                any_emitted_load = true;
            }
            if positions.unpinned.is_empty()
                && positions.pinned.is_empty()
                && positions.dedicated.is_empty()
            {
                continue;
            }
            for &index in positions
                .unpinned
                .iter()
                .chain(&positions.pinned)
                .chain(&positions.dedicated)
            {
                if index < block.instructions.len() {
                    loads[block_index].instructions.insert(index, private);
                    if !any_emitted_load
                        && loads_before(uses, shared_reload, span_closes, block_index, index)
                    {
                        any_emitted_load = true;
                    }
                }
            }
        }
        return Assignment {
            stores: vec![private; definitions.len()],
            loads,
            declares_private: !definitions.is_empty() || any_emitted_load,
            private,
        };
    }
    // Emitted load positions: one pair ahead of each span-start, pinned, or
    // dedicated position, plus the shared end-of-block point. The mid-span
    // unpinned positions that reuse the open pair read no slot of their own.
    let mut emitted: Vec<Vec<usize>> = vec![Vec::new(); block_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        // Use positions already name every possible reload. Unrelated
        // instructions cannot add an access to this victim's stream.
        let positions = &uses[block_index];
        if positions.unpinned.is_empty()
            && positions.pinned.is_empty()
            && positions.dedicated.is_empty()
        {
            continue;
        }
        let row = &mut emitted[block_index];
        for &index in positions
            .unpinned
            .iter()
            .chain(&positions.pinned)
            .chain(&positions.dedicated)
        {
            if index < block.instructions.len()
                && loads_before(uses, shared_reload, span_closes, block_index, index)
            {
                row.push(index);
            }
        }
        row.sort_unstable();
        row.dedup();
    }
    // Definitions anchored at each instruction index, in `definitions` order;
    // boundary definitions open their block ahead of every instruction.
    let mut anchored: Vec<std::collections::BTreeMap<usize, Vec<usize>>> =
        (0..block_count).map(|_| std::collections::BTreeMap::new()).collect();
    let mut boundary: Vec<Vec<usize>> = vec![Vec::new(); block_count];
    for (definition, storage) in definitions.iter().enumerate() {
        match storage.position {
            StoragePosition::BlockStart => boundary[storage.block_index].push(definition),
            StoragePosition::AfterInstruction(anchor)
            | StoragePosition::AfterUseDef {
                instruction: anchor,
                ..
            } => {
                if let Some(index) = function
                    .blocks
                    .instruction_index_in_block(storage.block_index, anchor)
                {
                    anchored[storage.block_index]
                        .entry(index)
                        .or_default()
                        .push(definition);
                }
            }
        }
    }
    // Each block's successor positions, resolved once for both fixpoints
    // below rather than by scanning the block list at every edge visit.
    let max_block_id = function
        .blocks
        .iter()
        .map(|block| block.id.0 as usize)
        .max()
        .unwrap_or(0);
    let mut block_positions = vec![u32::MAX; max_block_id + 1];
    for (block_index, block) in function.blocks.iter().enumerate() {
        block_positions[block.id.0 as usize] = block_index as u32;
    }
    let successors: Vec<Vec<usize>> = function
        .blocks
        .iter()
        .map(|block| {
            super::control(&block.terminator)
                .1
                .into_iter()
                .flatten()
                .filter_map(|successor| {
                    block_positions
                        .get(successor.block.0 as usize)
                        .copied()
                        .filter(|&pos| pos != u32::MAX)
                        .map(|pos| pos as usize)
                })
                .collect()
        })
        .collect();
    // Reaching-definition fixpoint over the victim's own store events: the
    // state entering a block is the union of every predecessor's exit, and a
    // store makes its own definition the only last writer. The state sampled
    // at a load position is the set of stores that could have written the
    // bytes that load must see — every member of it has to land in the same
    // slot, so the set merges into one window.
    let last_definitions: Vec<_> = (0..block_count)
        .map(|block| {
            anchored[block]
                .values()
                .rev()
                .find_map(|rows| rows.last().copied())
                .or_else(|| boundary[block].last().copied())
        })
        .collect();
    let entries =
        reaching::DefinitionEntries::compute(definitions.len(), &last_definitions, &successors);
    // Every position that reads the victim merges the definitions that can
    // reach it into one window — the load can name only one slot. Each block
    // is replayed once and the reaching set merged where a use samples it;
    // keeping a copy of the set at every instruction instead held one set per
    // instruction of the function and exhausted memory on a large function
    // whose victim has many definitions. A position reached by no definition
    // can arise only where dominance was already refused upstream; it merges
    // nothing and its window — store-free — fails every candidate, so it
    // resolves to the private slot.
    fn union(parent: &mut Vec<usize>, left: usize, right: usize) {
        let left = find(parent, left);
        let right = find(parent, right);
        parent[left] = right;
    }
    fn merge(parent: &mut Vec<usize>, state: &[u64]) -> Option<usize> {
        let mut members = reaching::members(state);
        let first = members.next()?;
        for other in members {
            union(parent, first, other);
        }
        Some(first)
    }
    let mut parent: Vec<usize> = (0..definitions.len()).collect();
    let mut window_of_position: std::collections::BTreeMap<(usize, usize), Option<usize>> =
        std::collections::BTreeMap::new();
    let mut window_of_end: Vec<Option<usize>> = vec![None; block_count];
    let mut state = entries.scratch();
    for (block_index, block) in function.blocks.iter().enumerate() {
        let positions = &uses[block_index];
        if positions.unpinned.is_empty()
            && positions.pinned.is_empty()
            && positions.dedicated.is_empty()
            && !positions.end_of_block
        {
            continue;
        }
        let sampled = positions
            .unpinned
            .iter()
            .chain(&positions.pinned)
            .chain(&positions.dedicated)
            .copied()
            .collect::<std::collections::BTreeSet<usize>>();
        entries.copy_into(block_index, &mut state);
        for &definition in &boundary[block_index] {
            reaching::replace(&mut state, definition);
        }
        let mut event_positions: Vec<_> = sampled
            .iter()
            .chain(anchored[block_index].keys())
            .copied()
            .filter(|index| *index < block.instructions.len())
            .collect();
        event_positions.sort_unstable();
        event_positions.dedup();
        for index in event_positions {
            if sampled.contains(&index) {
                window_of_position.insert((block_index, index), merge(&mut parent, &state));
            }
            if let Some(definitions_here) = anchored[block_index].get(&index) {
                for &definition in definitions_here {
                    reaching::replace(&mut state, definition);
                }
            }
        }
        if positions.end_of_block {
            window_of_end[block_index] = merge(&mut parent, &state);
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
    let mut classified_slots: Option<RawCandidateSlots> = None;
    let mut scratch = ShareableScratch::new(block_count);
    for (window, window_definitions) in windows.iter().enumerate() {
        // This window's emitted load positions.
        let mut load_marks: Vec<Vec<usize>> = vec![Vec::new(); block_count];
        let mut end_marks = vec![false; block_count];
        for (&(block_index, index), &mapped) in &window_of_position {
            let member = mapped
                .map(|root| *window_index.get(&root).unwrap_or(&windowless))
                .unwrap_or(windowless);
            if member == window && emitted[block_index].binary_search(&index).is_ok() {
                load_marks[block_index].push(index);
            }
        }
        for marks in &mut load_marks {
            if marks.len() > 1 {
                marks.sort_unstable();
                marks.dedup();
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
        if !candidate_slots.is_empty() {
            let classified = classified_slots
                .get_or_insert_with(|| RawCandidateSlots::scan(function, &candidate_slots));
            let summary = summarize_window(
                function,
                &boundary,
                &anchored,
                window_definitions,
                &load_marks,
                &end_marks,
            );
            for (slot_idx, &slot) in candidate_slots.iter().enumerate() {
                let Some(accesses) = classified.get(slot_idx) else {
                    continue;
                };
                if shareable(
                    accesses,
                    &summary,
                    &successors,
                    &load_marks,
                    &end_marks,
                    &mut scratch,
                ) {
                    chosen = Some(slot);
                    break;
                }
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

fn summarize_window(
    function: &SelectedFunction,
    boundary: &[Vec<usize>],
    anchored: &[std::collections::BTreeMap<usize, Vec<usize>>],
    window_definitions: &std::collections::BTreeSet<usize>,
    load_marks: &[Vec<usize>],
    end_marks: &[bool],
) -> WindowSummary {
    let block_count = function.blocks.len();
    let mut boundary_store = vec![false; block_count];
    let mut anchored_stores = vec![Vec::new(); block_count];
    let mut last_store = vec![None; block_count];
    let mut has_loads = vec![false; block_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        let has_boundary = boundary[block_index]
            .iter()
            .any(|definition| window_definitions.contains(definition));
        boundary_store[block_index] = has_boundary;
        let stores: Vec<usize> = anchored[block_index]
            .iter()
            .filter_map(|(&index, definitions)| {
                (index < block.instructions.len()
                    && definitions
                        .iter()
                        .any(|definition| window_definitions.contains(definition)))
                .then_some(index)
            })
            .collect();
        last_store[block_index] = stores
            .last()
            .copied()
            .map(StorePosition::Instruction)
            .or_else(|| has_boundary.then_some(StorePosition::Boundary));
        anchored_stores[block_index] = stores;
        has_loads[block_index] = !load_marks[block_index].is_empty() || end_marks[block_index];
    }
    WindowSummary {
        boundary_store,
        anchored_stores,
        last_store,
        has_loads,
    }
}

struct RawCandidateSlots {
    invalid: Vec<bool>,
    accesses: Vec<Vec<Vec<(usize, Event)>>>,
    terminal_loads: Vec<Vec<bool>>,
    finalized: Vec<Option<CandidateAccesses>>,
}

impl RawCandidateSlots {
    /// Scan all candidate slots' existing access streams in a single two-pass
    /// function scan, deferring per-slot sorting and summary allocation until
    /// `get(slot_idx)` is queried.
    fn scan(function: &SelectedFunction, candidates: &[LocalStorageSlotId]) -> Self {
        let count = candidates.len();
        if count == 0 {
            return Self {
                invalid: Vec::new(),
                accesses: Vec::new(),
                terminal_loads: Vec::new(),
                finalized: Vec::new(),
            };
        }
        let block_count = function.blocks.len();
        let max_spill_reg = candidates
            .iter()
            .filter_map(|slot| match slot {
                LocalStorageSlotId::Spill { register } => Some(register.0 as usize),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let mut slot_lookup = vec![u32::MAX; max_spill_reg + 1];
        for (idx, &slot) in candidates.iter().enumerate() {
            if let LocalStorageSlotId::Spill { register } = slot {
                slot_lookup[register.0 as usize] = idx as u32;
            }
        }
        let lookup_slot = |slot: LocalStorageSlotId| -> Option<usize> {
            match slot {
                LocalStorageSlotId::Spill { register } => slot_lookup
                    .get(register.0 as usize)
                    .copied()
                    .filter(|&idx| idx != u32::MAX)
                    .map(|idx| idx as usize),
                _ => None,
            }
        };

        let mut invalid = vec![false; count];
        let mut accesses: Vec<Vec<Vec<(usize, Event)>>> = vec![vec![Vec::new(); block_count]; count];
        let mut terminal_loads = vec![vec![false; block_count]; count];
        const NO_OWNER: u32 = u32::MAX;
        const MULTI_OWNER: u32 = u32::MAX - 1;
        let mut address_owner: Vec<u32> = Vec::new();
        let mut owner_bits: Vec<u64> = Vec::new();
        let mut any_address_owner = false;

        let invalidate = |slot: LocalStorageSlotId, invalid: &mut [bool]| {
            if let Some(idx) = lookup_slot(slot) {
                invalid[idx] = true;
            }
        };

        for block_index in 0..block_count {
            let Some(summary) = function.blocks.block_slot_summary(block_index) else {
                continue;
            };
            for &event in &summary.events {
                match event {
                    selected_instructions::BlockSlotEvent::Store {
                        slot: target,
                        index,
                    } => {
                        if let Some(slot_idx) = lookup_slot(target)
                            && !invalid[slot_idx]
                        {
                            accesses[slot_idx][block_index].push((index, Event::ExistingStore));
                        }
                    }
                    selected_instructions::BlockSlotEvent::FrameAddress {
                        slot: target,
                        register,
                    } => {
                        let reg_idx = register.0 as usize;
                        if reg_idx >= address_owner.len() {
                            address_owner.resize(reg_idx + 1, NO_OWNER);
                        }
                        let entry = &mut address_owner[reg_idx];
                        if let Some(slot_idx) = lookup_slot(target)
                            && !invalid[slot_idx]
                        {
                            if *entry == NO_OWNER {
                                *entry = slot_idx as u32;
                                let word = reg_idx >> 6;
                                if word >= owner_bits.len() {
                                    owner_bits.resize(word + 1, 0);
                                }
                                owner_bits[word] |= 1u64 << (reg_idx & 63);
                                any_address_owner = true;
                            } else if *entry != slot_idx as u32 {
                                if *entry < MULTI_OWNER {
                                    invalid[*entry as usize] = true;
                                }
                                invalid[slot_idx] = true;
                                *entry = MULTI_OWNER;
                            }
                        } else {
                            if *entry < MULTI_OWNER {
                                invalid[*entry as usize] = true;
                            }
                            *entry = MULTI_OWNER;
                        }
                    }
                    selected_instructions::BlockSlotEvent::Invalidate { slot: target } => {
                        invalidate(target, &mut invalid);
                    }
                }
            }
        }

        if any_address_owner {
            for block_index in 0..block_count {
                let Some(summary) = function.blocks.block_slot_summary(block_index) else {
                    continue;
                };
                for (word_idx, (&non_load_word, &owner_word)) in summary
                    .non_load_register_bits()
                    .iter()
                    .zip(&owner_bits)
                    .enumerate()
                {
                    let mut hit = non_load_word & owner_word;
                    while hit != 0 {
                        let bit = hit.trailing_zeros() as usize;
                        hit &= hit - 1;
                        let reg_idx = (word_idx << 6) | bit;
                        let owner = address_owner[reg_idx];
                        if owner < MULTI_OWNER {
                            invalid[owner as usize] = true;
                        }
                    }
                }
                for &(reg, index) in &summary.loads {
                    let Some(&owner) = address_owner.get(reg.0 as usize) else {
                        continue;
                    };
                    if owner >= MULTI_OWNER {
                        continue;
                    };
                    let slot_idx = owner as usize;
                    if invalid[slot_idx] {
                        continue;
                    }
                    if index == usize::MAX {
                        terminal_loads[slot_idx][block_index] = true;
                    } else {
                        accesses[slot_idx][block_index].push((index, Event::ExistingLoad));
                    }
                }
            }
        }

        Self {
            invalid,
            accesses,
            terminal_loads,
            finalized: (0..count).map(|_| None).collect(),
        }
    }

    fn get(&mut self, slot_idx: usize) -> Option<&CandidateAccesses> {
        if self.invalid[slot_idx] {
            return None;
        }
        if self.finalized[slot_idx].is_none() {
            let mut slot_accesses = std::mem::take(&mut self.accesses[slot_idx]);
            let slot_terminal_loads = std::mem::take(&mut self.terminal_loads[slot_idx]);
            for block_accesses in &mut slot_accesses {
                if block_accesses.len() > 1 {
                    block_accesses.sort_by_key(|&(idx, _)| idx);
                    block_accesses.dedup_by(|a, b| {
                        if a.0 == b.0 {
                            b.1 = a.1;
                            true
                        } else {
                            false
                        }
                    });
                }
            }
            let last_existing_store = slot_accesses
                .iter()
                .map(|block_accesses| {
                    block_accesses
                        .iter()
                        .rev()
                        .find_map(|&(index, event)| {
                            matches!(event, Event::ExistingStore).then_some(index)
                        })
                })
                .collect();
            let has_loads = slot_accesses
                .iter()
                .zip(&slot_terminal_loads)
                .map(|(block_accesses, &term_load)| {
                    term_load
                        || block_accesses
                            .iter()
                            .any(|&(_, event)| matches!(event, Event::ExistingLoad))
                })
                .collect();
            self.finalized[slot_idx] = Some(CandidateAccesses {
                accesses: slot_accesses,
                terminal_loads: slot_terminal_loads,
                last_existing_store,
                has_loads,
            });
        }
        self.finalized[slot_idx].as_ref()
    }
}

/// Whether one window's proposed accesses can share this candidate's slot:
/// a last-writer replay where `NEW` marks only this window's stores. Sibling
/// windows bound to the same slot are deliberately absent — a sibling store
/// can never be the last victim writer at this window's reloads, or the
/// reload would have joined that window instead.
fn shareable(
    candidate: &CandidateAccesses,
    window: &WindowSummary,
    successors: &[Vec<usize>],
    load_marks: &[Vec<usize>],
    end_marks: &[bool],
    scratch: &mut ShareableScratch,
) -> bool {
    let block_count = successors.len();
    scratch.pending.clear();
    for block_index in 0..block_count {
        scratch.entries[block_index] = 0;
        let exit = match (
            window.last_store[block_index],
            candidate.last_existing_store[block_index],
        ) {
            (Some(StorePosition::Instruction(w_idx)), Some(e_idx)) => {
                if e_idx > w_idx {
                    Some(EXISTING)
                } else {
                    Some(NEW)
                }
            }
            (Some(StorePosition::Boundary), Some(_)) => Some(EXISTING),
            (Some(_), None) => Some(NEW),
            (None, Some(_)) => Some(EXISTING),
            (None, None) => None,
        };
        scratch.exits[block_index] = exit;
        if exit.is_some() {
            scratch.queued[block_index] = true;
            scratch.pending.push_back(block_index);
        } else {
            scratch.queued[block_index] = false;
        }
    }
    while let Some(block) = scratch.pending.pop_front() {
        scratch.queued[block] = false;
        let exit = scratch.exits[block].unwrap_or(scratch.entries[block]);
        for &destination in &successors[block] {
            let merged = scratch.entries[destination] | exit;
            if merged == scratch.entries[destination] {
                continue;
            }
            scratch.entries[destination] = merged;
            if scratch.exits[destination].is_none() && !scratch.queued[destination] {
                scratch.queued[destination] = true;
                scratch.pending.push_back(destination);
            }
        }
    }
    for block_index in 0..block_count {
        if !window.has_loads[block_index] && !candidate.has_loads[block_index] {
            continue;
        }
        let mut state = scratch.entries[block_index];
        if window.boundary_store[block_index] {
            state = NEW;
        }
        let loads = &load_marks[block_index];
        let cand = &candidate.accesses[block_index];
        let stores = &window.anchored_stores[block_index];
        let mut i_load = 0;
        let mut i_cand = 0;
        let mut i_store = 0;
        loop {
            let next_load = loads.get(i_load).copied().unwrap_or(usize::MAX);
            let next_store = stores.get(i_store).copied().unwrap_or(usize::MAX);
            let next_victim = next_load.min(next_store);
            if state == EXISTING && i_cand < cand.len() && cand[i_cand].0 < next_victim {
                if next_victim == usize::MAX {
                    i_cand = cand.len();
                } else {
                    i_cand += cand[i_cand..].partition_point(|&(idx, _)| idx < next_victim);
                }
            }
            let next_cand = cand.get(i_cand).map(|&(idx, _)| idx).unwrap_or(usize::MAX);
            let index = next_victim.min(next_cand);
            if index == usize::MAX {
                break;
            }
            if next_load == index {
                if state != NEW {
                    return false;
                }
                i_load += 1;
            }
            if next_cand == index {
                match cand[i_cand].1 {
                    Event::ExistingStore => state = EXISTING,
                    Event::ExistingLoad => {
                        if state & NEW != 0 {
                            return false;
                        }
                    }
                }
                i_cand += 1;
            }
            if next_store == index {
                state = NEW;
                i_store += 1;
            }
        }
        if end_marks[block_index] && state != NEW {
            return false;
        }
        if candidate.terminal_loads[block_index] && state & NEW != 0 {
            return false;
        }
    }
    true
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
    span_closes[block_index]
        .range(previous..index)
        .next()
        .is_some()
}

/// The declared bit-preserving pair bridging a foreign-class victim to the
/// slot's carrier class. Only an IEEE-float payload has such a pair today;
/// each row must be the exact two-operand `Use`/`Def` bridge between the two
/// classes with no hidden effects — a conversion that wrote or clobbered a
/// unit could destroy the still-open reload it sits inside. A missing key, an
/// absent row, or a row that does not bridge the two classes is a
/// candidate-local rejection, not a constraint fault.
pub(super) fn bits_transport<'source>(
    environment: &'source ValidatedTargetRegisterEnvironment,
    victim: &VirtualRegister,
    carrier_class: RegisterClassId,
) -> Result<BitsConversion<'source>, RuntimeSpillError> {
    let keys = environment.selected_keys();
    let (to_key, from_key, to_kind, from_kind) = match victim.scalar_type {
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => (
            keys.float32_to_bits,
            keys.bits_to_float32,
            SelectedInstructionKind::Float32ToBits,
            SelectedInstructionKind::BitsToFloat32,
        ),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => (
            keys.float64_to_bits,
            keys.bits_to_float64,
            SelectedInstructionKind::Float64ToBits,
            SelectedInstructionKind::BitsToFloat64,
        ),
        _ => return Err(RuntimeSpillError::UnsupportedValue),
    };
    let to_bits = environment
        .constraint(to_key.ok_or(RuntimeSpillError::UnsupportedValue)?)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    let from_bits = environment
        .constraint(from_key.ok_or(RuntimeSpillError::UnsupportedValue)?)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    for (row, classes) in [
        (to_bits, [victim.class, carrier_class]),
        (from_bits, [carrier_class, victim.class]),
    ] {
        if row.operands.len() != 2 || !row.implicit_defs.is_empty() || !row.clobbers.is_empty() {
            return Err(RuntimeSpillError::UnsupportedValue);
        }
        for (ordinal, (operand, class)) in row.operands.iter().zip(classes).enumerate() {
            if usize::from(operand.operand) != ordinal
                || operand.access
                    != if ordinal == 0 {
                        RegisterOperandAccess::Use
                    } else {
                        RegisterOperandAccess::Def
                    }
                || operand.class != class
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber
            {
                return Err(RuntimeSpillError::UnsupportedValue);
            }
        }
    }
    Ok(BitsConversion {
        to_bits,
        from_bits,
        to_kind,
        from_kind,
    })
}

pub(super) fn entry_definitions(entry: usize, victim: &VirtualRegister) -> Vec<StorageDefinition> {
    vec![StorageDefinition {
        block_index: entry,
        position: StoragePosition::BlockStart,
        register: victim.id,
    }]
}

pub(super) fn physical_definition(
    function: &SelectedFunction,
    argument: VirtualRegisterId,
) -> Result<(usize, &SelectedInstruction), RuntimeSpillError> {
    let mut definitions = function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(owner, block)| {
            block.instructions.iter().flat_map(move |candidate| {
                candidate
                    .operands
                    .iter()
                    .filter(move |operand| {
                        operand.virtual_register == argument
                            && operand.access != RegisterOperandAccess::Use
                    })
                    .map(move |_| (owner, candidate))
            })
        });
    let Some(definition) = definitions.next() else {
        return Err(RuntimeSpillError::UnsupportedValue);
    };
    if definitions.next().is_some() {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    Ok(definition)
}
