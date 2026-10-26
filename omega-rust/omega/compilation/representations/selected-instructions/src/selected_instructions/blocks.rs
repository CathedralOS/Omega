//! Ordered selected blocks sharing immutable block contents.
//!
//! Recovery edits a few blocks of one function. Cloning that function must not
//! duplicate every instruction and operand vector in unrelated blocks, nor hash
//! them again. The roster is one vector of shared entries; each entry retains
//! its block and its derived memos together, and every mutable escape detaches
//! that entry and clears its memos — the roster contract `functions` carries
//! beside its own memos.
//!
//! No consumer holds a handle or observes an allocation address: neither is
//! compared, copied or serialized, so the roster carries no slot identity of
//! its own. The arena-and-span pair that preceded this stored the same rows
//! behind an occupancy sentinel whose vacant state was unreachable from an
//! occupied roster, resolved every read through a span, and rebuilt the whole
//! arena on `truncate` instead of dropping the tail.

use super::plan_identity::{CachedBlockEncoding, selected_block_digest_with_encoding};
use super::{
    SelectedBlock, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
    VirtualRegisterId,
};
use std::fmt;
use std::ops::{Index, IndexMut};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedBlocks(Vec<Arc<BlockEntry>>);

/// One block and the memoized derivations of its content: the digest of its
/// canonical identity encoding and the mechanical lookup index. Each memo is a
/// pure function of the block beside it; every mutable access detaches the
/// entry and clears both before handing out the block. Equality compares the
/// block alone, like the storage. Public only so the storage iterator types
/// can name it; this module is private.
#[derive(Clone)]
pub struct BlockEntry {
    block: SelectedBlock,
    digest: OnceLock<[u8; 32]>,
    encoding: OnceLock<Arc<CachedBlockEncoding>>,
    previous_encoding: Option<(Arc<CachedBlockEncoding>, usize, usize)>,
    lookup: OnceLock<BlockLookup>,
    slot_summary: OnceLock<BlockSlotSummary>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockSlotEvent {
    Store {
        slot: super::LocalStorageSlotId,
        index: usize,
    },
    FrameAddress {
        slot: super::LocalStorageSlotId,
        register: VirtualRegisterId,
    },
    Invalidate {
        slot: super::LocalStorageSlotId,
    },
}

#[derive(Clone, Debug, Default)]
pub struct BlockSlotSummary {
    pub events: Vec<BlockSlotEvent>,
    pub loads: Vec<(VirtualRegisterId, usize)>,
    non_load_register_bits: Vec<u64>,
}

impl BlockSlotSummary {
    #[inline]
    pub fn has_non_load_register(&self, register: VirtualRegisterId) -> bool {
        let word = (register.0 as usize) >> 6;
        self.non_load_register_bits
            .get(word)
            .is_some_and(|&bits| (bits & (1u64 << (register.0 & 63))) != 0)
    }

    #[inline]
    pub fn non_load_register_bits(&self) -> &[u64] {
        &self.non_load_register_bits
    }
}

#[inline]
fn set_u32_bit(bits: &mut Vec<u64>, id: u32) {
    let word = (id as usize) >> 6;
    if word >= bits.len() {
        bits.resize(word + 1, 0);
    }
    bits[word] |= 1u64 << (id & 63);
}

#[inline]
fn set_register_bit(bits: &mut Vec<u64>, register: VirtualRegisterId) {
    set_u32_bit(bits, register.0);
}

#[derive(Clone)]
struct BlockLookup {
    fixed_definition_bits: Vec<u64>,
    materialize_definition_bits: Vec<u64>,
    maximum_instruction: SelectedInstructionId,
    operand_count: Option<usize>,
    transport_count: Option<usize>,
    implicit_use_units: Vec<register_model::RegisterUnitId>,
    upward_unit_uses: Vec<register_model::RegisterUnitId>,
    unit_kills: Vec<register_model::RegisterUnitId>,
    unit_writer_indices: Vec<usize>,
    fixed_views: Vec<register_model::RegisterViewId>,
    instruction_bits: Vec<u64>,
    instruction_ids: Vec<u32>,
    referenced_register_bits: Vec<u64>,
    register_instruction_spans: Vec<(u32, u32)>,
}
impl BlockEntry {
    fn new(block: SelectedBlock) -> Arc<Self> {
        Arc::new(Self {
            block,
            digest: OnceLock::new(),
            encoding: OnceLock::new(),
            previous_encoding: None,
            lookup: OnceLock::new(),
            slot_summary: OnceLock::new(),
        })
    }
    fn new_with_previous_encoding(
        block: SelectedBlock,
        previous_encoding: Option<(Arc<CachedBlockEncoding>, usize, usize)>,
    ) -> Arc<Self> {
        Arc::new(Self {
            block,
            digest: OnceLock::new(),
            encoding: OnceLock::new(),
            previous_encoding,
            lookup: OnceLock::new(),
            slot_summary: OnceLock::new(),
        })
    }
    fn digest(&self) -> [u8; 32] {
        *self.digest.get_or_init(|| {
            let (digest, encoding) = selected_block_digest_with_encoding(
                &self.block,
                self.previous_encoding
                    .as_ref()
                    .map(|(enc, prefix, suffix)| (enc.as_ref(), *prefix, *suffix)),
            );
            let _ = self.encoding.set(encoding);
            digest
        })
    }
    fn lookup(&self) -> &BlockLookup {
        self.lookup.get_or_init(|| {
            let mut fixed_definition_bits = Vec::new();
            let mut materialize_definition_bits = Vec::new();
            let mut implicit_use_mask = 0u64;
            let mut upward_unit_uses_mask = 0u64;
            let mut unit_kills_mask = 0u64;
            let mut fixed_views_mask = 0u64;
            let mut implicit_use_units = Vec::new();
            let mut upward_unit_uses = Vec::new();
            let mut unit_kills = Vec::new();
            let mut unit_writer_indices = Vec::new();
            let mut fixed_views = Vec::new();
            let bitset_words = (self.block.instructions.len() / 16).max(4);
            let mut instruction_bits = Vec::with_capacity(bitset_words);
            let mut instruction_ids = Vec::with_capacity(self.block.instructions.len());
            let mut referenced_register_bits = Vec::with_capacity(bitset_words);
            let record_spans = self.block.instructions.len() >= 64;
            let mut register_instruction_spans: Vec<(u32, u32)> = if record_spans {
                Vec::with_capacity(self.block.instructions.len())
            } else {
                Vec::new()
            };
            let terminator = super::terminator_instruction(&self.block.terminator);
            let mut maximum = terminator.id;
            let mut operand_count = Some(terminator.operands.len());
            for (index, instruction) in self.block.instructions.iter().enumerate() {
                maximum = maximum.max(instruction.id);
                operand_count =
                    operand_count.and_then(|total| total.checked_add(instruction.operands.len()));
                set_u32_bit(&mut instruction_bits, instruction.id.0);
                instruction_ids.push(instruction.id.0);
                for &unit in &instruction.implicit_uses {
                    if (unit.0 as usize) < 64 {
                        let bit = 1u64 << unit.0;
                        implicit_use_mask |= bit;
                        if (unit_kills_mask & bit) == 0 {
                            upward_unit_uses_mask |= bit;
                        }
                    } else {
                        implicit_use_units.push(unit);
                        if !unit_kills.contains(&unit) && !upward_unit_uses.contains(&unit) {
                            upward_unit_uses.push(unit);
                        }
                    }
                }
                for &unit in instruction.implicit_defs.iter().chain(&instruction.clobbers) {
                    if (unit.0 as usize) < 64 {
                        unit_kills_mask |= 1u64 << unit.0;
                    } else if !unit_kills.contains(&unit) {
                        unit_kills.push(unit);
                    }
                }
                if !instruction.clobbers.is_empty() || !instruction.implicit_defs.is_empty() {
                    unit_writer_indices.push(index);
                }
                if matches!(
                    instruction.kind,
                    super::SelectedInstructionKind::MaterializeI64 { .. }
                ) && let [operand] = instruction.operands.as_slice()
                    && operand.access == register_model::RegisterOperandAccess::Def
                {
                    set_register_bit(&mut materialize_definition_bits, operand.virtual_register);
                }
                for operand in &instruction.operands {
                    set_register_bit(&mut referenced_register_bits, operand.virtual_register);
                    if record_spans {
                        let reg_idx = operand.virtual_register.0 as usize;
                        if reg_idx >= register_instruction_spans.len() {
                            register_instruction_spans.resize(reg_idx + 1, (u32::MAX, 0));
                        }
                        let idx_u32 = index as u32;
                        let span = &mut register_instruction_spans[reg_idx];
                        if span.0 == u32::MAX {
                            *span = (idx_u32, idx_u32);
                        } else {
                            span.1 = idx_u32;
                        }
                    }
                    if let Some(view) = operand.fixed_view {
                        if (view.0 as usize) < 64 {
                            fixed_views_mask |= 1u64 << view.0;
                        } else {
                            fixed_views.push(view);
                        }
                        if operand.access != register_model::RegisterOperandAccess::Use {
                            set_register_bit(&mut fixed_definition_bits, operand.virtual_register);
                        }
                    }
                }
            }
            for &unit in &terminator.implicit_uses {
                if (unit.0 as usize) < 64 {
                    let bit = 1u64 << unit.0;
                    implicit_use_mask |= bit;
                    if (unit_kills_mask & bit) == 0 {
                        upward_unit_uses_mask |= bit;
                    }
                } else {
                    implicit_use_units.push(unit);
                    if !unit_kills.contains(&unit) && !upward_unit_uses.contains(&unit) {
                        upward_unit_uses.push(unit);
                    }
                }
            }
            for &unit in terminator.implicit_defs.iter().chain(&terminator.clobbers) {
                if (unit.0 as usize) < 64 {
                    unit_kills_mask |= 1u64 << unit.0;
                } else if !unit_kills.contains(&unit) {
                    unit_kills.push(unit);
                }
            }
            for operand in &terminator.operands {
                set_register_bit(&mut referenced_register_bits, operand.virtual_register);
                if let Some(view) = operand.fixed_view {
                    if (view.0 as usize) < 64 {
                        fixed_views_mask |= 1u64 << view.0;
                    } else {
                        fixed_views.push(view);
                    }
                }
            }
            let mut transport_count = Some(0usize);
            for successor in super::terminator_successors(&self.block.terminator) {
                let edge_transports = successor
                    .bindings
                    .len()
                    .checked_add(successor.structural_bindings.len())
                    .and_then(|total| {
                        total.checked_add(
                            successor
                                .structural_case
                                .as_ref()
                                .map_or(0, |case| case.payloads.len().saturating_add(1)),
                        )
                    });
                transport_count = transport_count
                    .zip(edge_transports)
                    .and_then(|(total, edge)| total.checked_add(edge));
                for binding in &successor.bindings {
                    if let super::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } = binding.transport
                    {
                        set_register_bit(&mut referenced_register_bits, argument);
                        set_register_bit(&mut referenced_register_bits, parameter);
                    }
                }
                for binding in &successor.structural_bindings {
                    match binding.transport {
                        super::SelectedStructuralTransport::WholeValue {
                            argument,
                            ..
                        } => {
                            set_register_bit(&mut referenced_register_bits, argument);
                        }
                        super::SelectedStructuralTransport::ScalarReference {
                            base: super::SelectedAddressBase::Register(argument),
                            ..
                        }
                        | super::SelectedStructuralTransport::Address {
                            base: super::SelectedAddressBase::Register(argument),
                            ..
                        } => {
                            set_register_bit(&mut referenced_register_bits, argument);
                        }
                        _ => {}
                    }
                }
                if let Some(case) = &successor.structural_case {
                    if let super::SelectedCaseDispatchSource::Borrowed { pointer, .. } = case.source
                    {
                        set_register_bit(&mut referenced_register_bits, pointer);
                    }
                    for payload in &case.payloads {
                        match payload.transport {
                            super::SelectedCasePayloadTransport::Registers {
                                argument,
                                parameter,
                            } => {
                                set_register_bit(&mut referenced_register_bits, argument);
                                set_register_bit(&mut referenced_register_bits, parameter);
                            }
                            super::SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                                set_register_bit(&mut referenced_register_bits, parameter);
                            }
                            super::SelectedCasePayloadTransport::Unused => {}
                        }
                    }
                }
            }
            let mut m = implicit_use_mask;
            while m != 0 {
                let bit = m.trailing_zeros() as u16;
                m &= m - 1;
                implicit_use_units.push(register_model::RegisterUnitId(bit));
            }
            if implicit_use_units.len() > implicit_use_mask.count_ones() as usize {
                implicit_use_units.sort_unstable();
                implicit_use_units.dedup();
            }
            let mut m = upward_unit_uses_mask;
            while m != 0 {
                let bit = m.trailing_zeros() as u16;
                m &= m - 1;
                upward_unit_uses.push(register_model::RegisterUnitId(bit));
            }
            let mut m = unit_kills_mask;
            while m != 0 {
                let bit = m.trailing_zeros() as u16;
                m &= m - 1;
                unit_kills.push(register_model::RegisterUnitId(bit));
            }
            let mut m = fixed_views_mask;
            while m != 0 {
                let bit = m.trailing_zeros() as u16;
                m &= m - 1;
                fixed_views.push(register_model::RegisterViewId(bit));
            }
            if fixed_views.len() > fixed_views_mask.count_ones() as usize {
                fixed_views.sort_unstable();
                fixed_views.dedup();
            }
            BlockLookup {
                fixed_definition_bits,
                materialize_definition_bits,
                maximum_instruction: maximum,
                operand_count,
                transport_count,
                implicit_use_units,
                upward_unit_uses,
                unit_kills,
                unit_writer_indices,
                fixed_views,
                instruction_bits,
                instruction_ids,
                referenced_register_bits,
                register_instruction_spans,
            }
        })
    }
    fn slot_summary(&self) -> &BlockSlotSummary {
        self.slot_summary.get_or_init(|| {
            let bitset_words = (self.block.instructions.len() / 16).max(4);
            let mut slot_summary = BlockSlotSummary {
                events: Vec::new(),
                loads: Vec::new(),
                non_load_register_bits: Vec::with_capacity(bitset_words),
            };
            for (index, instruction) in self.block.instructions.iter().enumerate() {
                match instruction.kind {
                    super::SelectedInstructionKind::Store64 {
                        slot: super::FrameStorageSlotId::Local(target),
                        byte_offset,
                    } => {
                        if byte_offset != 0 {
                            slot_summary
                                .events
                                .push(BlockSlotEvent::Invalidate { slot: target });
                        } else {
                            slot_summary
                                .events
                                .push(BlockSlotEvent::Store { slot: target, index });
                        }
                        for operand in &instruction.operands {
                            set_register_bit(
                                &mut slot_summary.non_load_register_bits,
                                operand.virtual_register,
                            );
                        }
                    }
                    super::SelectedInstructionKind::FrameAddress {
                        slot: super::FrameStorageSlotId::Local(target),
                        byte_offset,
                    } => {
                        if byte_offset != 0 {
                            slot_summary
                                .events
                                .push(BlockSlotEvent::Invalidate { slot: target });
                            for operand in &instruction.operands {
                                set_register_bit(
                                    &mut slot_summary.non_load_register_bits,
                                    operand.virtual_register,
                                );
                            }
                        } else {
                            for operand in &instruction.operands {
                                if operand.access != register_model::RegisterOperandAccess::Use {
                                    slot_summary.events.push(BlockSlotEvent::FrameAddress {
                                        slot: target,
                                        register: operand.virtual_register,
                                    });
                                } else {
                                    set_register_bit(
                                        &mut slot_summary.non_load_register_bits,
                                        operand.virtual_register,
                                    );
                                }
                            }
                        }
                    }
                    super::SelectedInstructionKind::HostedReadByte { slot: target }
                    | super::SelectedInstructionKind::HostedWriteByteI32 { slot: target } => {
                        slot_summary
                            .events
                            .push(BlockSlotEvent::Invalidate { slot: target });
                        for operand in &instruction.operands {
                            set_register_bit(
                                &mut slot_summary.non_load_register_bits,
                                operand.virtual_register,
                            );
                        }
                    }
                    super::SelectedInstructionKind::Load64 { byte_offset: 0 } => {
                        for operand in &instruction.operands {
                            if operand.access == register_model::RegisterOperandAccess::Use {
                                slot_summary.loads.push((operand.virtual_register, index));
                            } else {
                                set_register_bit(
                                    &mut slot_summary.non_load_register_bits,
                                    operand.virtual_register,
                                );
                            }
                        }
                    }
                    _ => {
                        for operand in &instruction.operands {
                            set_register_bit(
                                &mut slot_summary.non_load_register_bits,
                                operand.virtual_register,
                            );
                        }
                    }
                }
            }
            let terminator = super::terminator_instruction(&self.block.terminator);
            match terminator.kind {
                super::SelectedInstructionKind::Store64 {
                    slot: super::FrameStorageSlotId::Local(target),
                    ..
                }
                | super::SelectedInstructionKind::FrameAddress {
                    slot: super::FrameStorageSlotId::Local(target),
                    ..
                }
                | super::SelectedInstructionKind::HostedReadByte { slot: target }
                | super::SelectedInstructionKind::HostedWriteByteI32 { slot: target } => {
                    slot_summary
                        .events
                        .push(BlockSlotEvent::Invalidate { slot: target });
                }
                _ => {}
            }
            for operand in &terminator.operands {
                if matches!(
                    terminator.kind,
                    super::SelectedInstructionKind::Load64 { byte_offset: 0 }
                ) && operand.access == register_model::RegisterOperandAccess::Use
                {
                    slot_summary
                        .loads
                        .push((operand.virtual_register, usize::MAX));
                } else {
                    set_register_bit(
                        &mut slot_summary.non_load_register_bits,
                        operand.virtual_register,
                    );
                }
            }
            for successor in super::terminator_successors(&self.block.terminator) {
                for binding in &successor.bindings {
                    if let super::SelectedValueTransport::Registers { argument, .. } =
                        binding.transport
                    {
                        set_register_bit(&mut slot_summary.non_load_register_bits, argument);
                    }
                }
                for binding in &successor.structural_bindings {
                    match binding.transport {
                        super::SelectedStructuralTransport::WholeValue {
                            destination,
                            argument,
                            ..
                        } => {
                            slot_summary
                                .events
                                .push(BlockSlotEvent::Invalidate { slot: destination });
                            set_register_bit(&mut slot_summary.non_load_register_bits, argument);
                        }
                        super::SelectedStructuralTransport::ScalarReference {
                            base,
                            destination,
                            ..
                        }
                        | super::SelectedStructuralTransport::Address {
                            base,
                            destination,
                            ..
                        } => {
                            slot_summary
                                .events
                                .push(BlockSlotEvent::Invalidate { slot: destination });
                            match base {
                                super::SelectedAddressBase::Local(base_slot) => {
                                    slot_summary
                                        .events
                                        .push(BlockSlotEvent::Invalidate { slot: base_slot });
                                }
                                super::SelectedAddressBase::Register(argument) => {
                                    set_register_bit(
                                        &mut slot_summary.non_load_register_bits,
                                        argument,
                                    );
                                }
                            }
                        }
                        super::SelectedStructuralTransport::Unused => {}
                    }
                }
                if let Some(case) = &successor.structural_case {
                    if let Some(case_slot) = case.source.local_slot() {
                        slot_summary
                            .events
                            .push(BlockSlotEvent::Invalidate { slot: case_slot });
                    }
                    for payload in &case.payloads {
                        if let super::SelectedCasePayloadTransport::Registers { argument, .. } =
                            payload.transport
                        {
                            set_register_bit(
                                &mut slot_summary.non_load_register_bits,
                                argument,
                            );
                        }
                    }
                }
            }
            slot_summary
        })
    }
}
impl PartialEq for BlockEntry {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
    }
}
impl Eq for BlockEntry {}
/// Shows the block alone, like equality.
impl fmt::Debug for BlockEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.block, formatter)
    }
}
/// The only route to a mutable block: detach a shared entry and drop its
/// memos, which no longer describe what the caller may write.
fn detach(entry: &mut Arc<BlockEntry>) -> &mut SelectedBlock {
    let entry = Arc::make_mut(entry);
    entry.digest.take();
    entry.encoding.take();
    entry.previous_encoding = None;
    entry.lookup.take();
    entry.slot_summary.take();
    &mut entry.block
}
fn block_of(entry: &Arc<BlockEntry>) -> &SelectedBlock {
    &entry.block
}
impl SelectedBlocks {
    /// A mechanical negative filter for split admission. Positive candidates
    /// still undergo the complete source/constraint/use audit at its owner.
    pub fn has_fixed_definition(&self, register: VirtualRegisterId) -> bool {
        let word = (register.0 as usize) >> 6;
        let mask = 1u64 << (register.0 & 63);
        self.0.iter().any(|entry| {
            entry
                .lookup()
                .fixed_definition_bits
                .get(word)
                .is_some_and(|&bits| (bits & mask) != 0)
        })
    }
    /// A mechanical negative filter for rematerialization admission. Positive
    /// candidates still undergo the complete source/constraint/use audit at its owner.
    pub fn has_materialize_definition(&self, register: VirtualRegisterId) -> bool {
        let word = (register.0 as usize) >> 6;
        let mask = 1u64 << (register.0 & 63);
        self.0.iter().any(|entry| {
            entry
                .lookup()
                .materialize_definition_bits
                .get(word)
                .is_some_and(|&bits| (bits & mask) != 0)
        })
    }
    pub fn maximum_instruction_id(&self) -> Option<SelectedInstructionId> {
        self.0
            .iter()
            .map(|entry| entry.lookup().maximum_instruction)
            .max()
    }
    /// Total operand count across all body instructions and terminators of this block roster.
    pub fn total_operand_count(&self) -> Option<usize> {
        self.0.iter().try_fold(0usize, |total, entry| {
            total.checked_add(entry.lookup().operand_count?)
        })
    }
    /// Total edge-transport surface across all block terminators of this roster.
    pub fn total_transport_count(&self) -> Option<usize> {
        self.0.iter().try_fold(0usize, |total, entry| {
            total.checked_add(entry.lookup().transport_count?)
        })
    }
    /// Maximum 64-bit word count of referenced register bitsets across all blocks.
    pub fn max_referenced_register_word_count(&self) -> usize {
        self.0
            .iter()
            .map(|entry| entry.lookup().referenced_register_bits.len())
            .max()
            .unwrap_or(0)
    }
    /// Inclusive `(first_instruction_index, last_instruction_index)` in `block_index`'s
    /// body instructions where `register` appears as an operand, or `None` when no
    /// body instruction in `block_index` references `register`.
    pub fn block_register_instruction_span(
        &self,
        block_index: usize,
        register: VirtualRegisterId,
    ) -> Option<(usize, usize)> {
        let entry = self.0.get(block_index)?;
        let len = entry.block.instructions.len();
        if len == 0 {
            return None;
        }
        let lookup = entry.lookup();
        let word = (register.0 as usize) >> 6;
        if !lookup
            .referenced_register_bits
            .get(word)
            .is_some_and(|&w| (w & (1u64 << (register.0 & 63))) != 0)
        {
            return None;
        }
        if len >= 64 {
            let &(first, last) = lookup
                .register_instruction_spans
                .get(register.0 as usize)?;
            if first == u32::MAX {
                None
            } else {
                Some((first as usize, last as usize))
            }
        } else {
            Some((0, len - 1))
        }
    }
    /// Replace `index`'s instructions and terminator without cloning the old
    /// `SelectedBlock` when the entry is shared, optionally retaining the first
    /// `unchanged_instruction_prefix` and last `unchanged_instruction_suffix`
    /// instructions' pre-encoded digest chunks.
    pub fn replace_block_contents(
        &mut self,
        index: usize,
        instructions: Vec<SelectedInstruction>,
        terminator: SelectedTerminator,
        unchanged_instruction_prefix: usize,
        unchanged_instruction_suffix: usize,
    ) {
        let entry = &self.0[index];
        let id = entry.block.id;
        let origin = entry.block.origin;
        let previous_encoding = entry.encoding.get().cloned().map(|enc| {
            (
                enc,
                unchanged_instruction_prefix,
                unchanged_instruction_suffix,
            )
        });
        self.0[index] = BlockEntry::new_with_previous_encoding(
            SelectedBlock {
                id,
                origin,
                instructions,
                terminator,
            },
            previous_encoding,
        );
    }
    /// Block index containing the body instruction `instruction`, using each
    /// block's memoized instruction index.
    pub fn instruction_block(&self, instruction: SelectedInstructionId) -> Option<usize> {
        let word = (instruction.0 as usize) >> 6;
        let mask = 1u64 << (instruction.0 & 63);
        self.0.iter().position(|entry| {
            entry
                .lookup()
                .instruction_bits
                .get(word)
                .is_some_and(|&bits| (bits & mask) != 0)
        })
    }
    /// Instruction index inside `block_index` for body instruction `instruction`.
    pub fn instruction_index_in_block(
        &self,
        block_index: usize,
        instruction: SelectedInstructionId,
    ) -> Option<usize> {
        let lookup = self.0.get(block_index)?.lookup();
        let word = (instruction.0 as usize) >> 6;
        if !lookup
            .instruction_bits
            .get(word)
            .is_some_and(|&bits| (bits & (1u64 << (instruction.0 & 63))) != 0)
        {
            return None;
        }
        lookup
            .instruction_ids
            .iter()
            .position(|&id| id == instruction.0)
    }
    /// Memoized implicit-use units and fixed views per block.
    pub fn block_implicit_units_and_fixed_views(
        &self,
        block_index: usize,
    ) -> Option<(&[register_model::RegisterUnitId], &[register_model::RegisterViewId])> {
        let lookup = self.0.get(block_index)?.lookup();
        Some((&lookup.implicit_use_units, &lookup.fixed_views))
    }
    /// Memoized upward-exposed unit uses and unit kills per block.
    pub fn block_unit_transfer(
        &self,
        block_index: usize,
    ) -> Option<(&[register_model::RegisterUnitId], &[register_model::RegisterUnitId])> {
        let lookup = self.0.get(block_index)?.lookup();
        Some((&lookup.upward_unit_uses, &lookup.unit_kills))
    }
    /// Memoized body instruction indices that clobber or implicitly define at least one unit.
    pub fn block_unit_writer_indices(&self, block_index: usize) -> Option<&[usize]> {
        Some(&self.0.get(block_index)?.lookup().unit_writer_indices)
    }
    /// Memoized slot access summary per block.
    pub fn block_slot_summary(&self, block_index: usize) -> Option<&BlockSlotSummary> {
        Some(self.0.get(block_index)?.slot_summary())
    }
    /// Whether `block_index` references `register` in any instruction, terminator, or edge transport.
    pub fn block_references_register(&self, block_index: usize, register: VirtualRegisterId) -> bool {
        self.0.get(block_index).is_some_and(|entry| {
            let bits = &entry.lookup().referenced_register_bits;
            let word = (register.0 as usize) >> 6;
            bits.get(word)
                .is_some_and(|&w| (w & (1u64 << (register.0 & 63))) != 0)
        })
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&SelectedBlock> {
        self.0.get(index).map(block_of)
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut SelectedBlock> {
        self.0.get_mut(index).map(detach)
    }
    pub fn first(&self) -> Option<&SelectedBlock> {
        self.get(0)
    }
    pub fn last(&self) -> Option<&SelectedBlock> {
        self.0.last().map(block_of)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SelectedBlock> + DoubleEndedIterator {
        self.0.iter().map(block_of)
    }
    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = &mut SelectedBlock> + DoubleEndedIterator {
        self.0.iter_mut().map(detach)
    }
    pub fn push(&mut self, block: SelectedBlock) {
        self.0.push(BlockEntry::new(block));
    }
    pub fn extend(&mut self, blocks: impl IntoIterator<Item = SelectedBlock>) {
        self.0.extend(blocks.into_iter().map(BlockEntry::new));
    }
    /// Drops the tail in place; the retained prefix keeps its shared entries.
    pub fn truncate(&mut self, length: usize) {
        self.0.truncate(length);
    }
    pub fn shares_block_storage(&self, other: &Self, index: usize) -> bool {
        self.0
            .get(index)
            .zip(other.0.get(index))
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
    pub(crate) fn identity_digests(&self) -> impl ExactSizeIterator<Item = [u8; 32]> + '_ {
        self.0.iter().map(|entry| entry.digest())
    }
}
impl From<Vec<SelectedBlock>> for SelectedBlocks {
    fn from(blocks: Vec<SelectedBlock>) -> Self {
        blocks.into_iter().collect()
    }
}
impl FromIterator<SelectedBlock> for SelectedBlocks {
    fn from_iter<T: IntoIterator<Item = SelectedBlock>>(blocks: T) -> Self {
        Self(blocks.into_iter().map(BlockEntry::new).collect())
    }
}
impl Index<usize> for SelectedBlocks {
    type Output = SelectedBlock;

    fn index(&self, index: usize) -> &Self::Output {
        block_of(&self.0[index])
    }
}
impl IndexMut<usize> for SelectedBlocks {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        detach(&mut self.0[index])
    }
}
impl<'storage> IntoIterator for &'storage SelectedBlocks {
    type Item = &'storage SelectedBlock;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'storage, Arc<BlockEntry>>,
        fn(&'storage Arc<BlockEntry>) -> &'storage SelectedBlock,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(block_of)
    }
}
impl<'storage> IntoIterator for &'storage mut SelectedBlocks {
    type Item = &'storage mut SelectedBlock;
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'storage, Arc<BlockEntry>>,
        fn(&'storage mut Arc<BlockEntry>) -> &'storage mut SelectedBlock,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut().map(detach)
    }
}
