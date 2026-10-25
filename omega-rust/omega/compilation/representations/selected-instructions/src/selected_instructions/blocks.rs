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

use super::plan_identity::selected_block_digest;
use super::{SelectedBlock, SelectedInstructionId, VirtualRegisterId};
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
    lookup: OnceLock<BlockLookup>,
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
fn set_register_bit(bits: &mut Vec<u64>, register: VirtualRegisterId) {
    let word = (register.0 as usize) >> 6;
    if word >= bits.len() {
        bits.resize(word + 1, 0);
    }
    bits[word] |= 1u64 << (register.0 & 63);
}

#[derive(Clone)]
struct BlockLookup {
    fixed_definitions: Vec<VirtualRegisterId>,
    maximum_instruction: SelectedInstructionId,
    operand_count: Option<usize>,
    implicit_use_units: Vec<register_model::RegisterUnitId>,
    upward_unit_uses: Vec<register_model::RegisterUnitId>,
    unit_kills: Vec<register_model::RegisterUnitId>,
    unit_writer_indices: Vec<usize>,
    fixed_views: Vec<register_model::RegisterViewId>,
    instructions_sorted: Vec<(SelectedInstructionId, usize)>,
    referenced_register_bits: Vec<u64>,
    slot_summary: BlockSlotSummary,
}
impl BlockEntry {
    fn new(block: SelectedBlock) -> Arc<Self> {
        Arc::new(Self {
            block,
            digest: OnceLock::new(),
            lookup: OnceLock::new(),
        })
    }
    fn digest(&self) -> [u8; 32] {
        *self
            .digest
            .get_or_init(|| selected_block_digest(&self.block))
    }
    fn lookup(&self) -> &BlockLookup {
        self.lookup.get_or_init(|| {
            let mut fixed_definitions = Vec::new();
            let mut implicit_use_units = Vec::new();
            let mut upward_unit_uses = Vec::new();
            let mut unit_kills = Vec::new();
            let mut unit_writer_indices = Vec::new();
            let mut fixed_views = Vec::new();
            let mut instructions_sorted = Vec::with_capacity(self.block.instructions.len());
            let bitset_words = (self.block.instructions.len() / 16).max(4);
            let mut referenced_register_bits = Vec::with_capacity(bitset_words);
            let mut slot_summary = BlockSlotSummary {
                events: Vec::new(),
                loads: Vec::new(),
                non_load_register_bits: Vec::with_capacity(bitset_words),
            };
            let terminator = super::terminator_instruction(&self.block.terminator);
            let mut maximum = terminator.id;
            let mut operand_count = Some(terminator.operands.len());
            for (index, instruction) in self.block.instructions.iter().enumerate() {
                maximum = maximum.max(instruction.id);
                operand_count =
                    operand_count.and_then(|total| total.checked_add(instruction.operands.len()));
                instructions_sorted.push((instruction.id, index));
                implicit_use_units.extend_from_slice(&instruction.implicit_uses);
                for &unit in &instruction.implicit_uses {
                    if !unit_kills.contains(&unit) && !upward_unit_uses.contains(&unit) {
                        upward_unit_uses.push(unit);
                    }
                }
                for &unit in instruction.implicit_defs.iter().chain(&instruction.clobbers) {
                    if !unit_kills.contains(&unit) {
                        unit_kills.push(unit);
                    }
                }
                if !instruction.clobbers.is_empty() || !instruction.implicit_defs.is_empty() {
                    unit_writer_indices.push(index);
                }
                for operand in &instruction.operands {
                    set_register_bit(&mut referenced_register_bits, operand.virtual_register);
                    if let Some(view) = operand.fixed_view {
                        fixed_views.push(view);
                        if operand.access != register_model::RegisterOperandAccess::Use {
                            fixed_definitions.push(operand.virtual_register);
                        }
                    }
                }
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
            implicit_use_units.extend_from_slice(&terminator.implicit_uses);
            for &unit in &terminator.implicit_uses {
                if !unit_kills.contains(&unit) && !upward_unit_uses.contains(&unit) {
                    upward_unit_uses.push(unit);
                }
            }
            for &unit in terminator.implicit_defs.iter().chain(&terminator.clobbers) {
                if !unit_kills.contains(&unit) {
                    unit_kills.push(unit);
                }
            }
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
                set_register_bit(&mut referenced_register_bits, operand.virtual_register);
                if let Some(view) = operand.fixed_view {
                    fixed_views.push(view);
                }
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
                    if let super::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } = binding.transport
                    {
                        set_register_bit(&mut referenced_register_bits, argument);
                        set_register_bit(&mut referenced_register_bits, parameter);
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
                            set_register_bit(&mut referenced_register_bits, argument);
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
                                    set_register_bit(&mut referenced_register_bits, argument);
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
                                set_register_bit(
                                    &mut slot_summary.non_load_register_bits,
                                    argument,
                                );
                            }
                            super::SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                                set_register_bit(&mut referenced_register_bits, parameter);
                            }
                            super::SelectedCasePayloadTransport::Unused => {}
                        }
                    }
                }
            }
            fixed_definitions.sort_unstable();
            fixed_definitions.dedup();
            implicit_use_units.sort_unstable();
            implicit_use_units.dedup();
            fixed_views.sort_unstable();
            fixed_views.dedup();
            if instructions_sorted.windows(2).any(|w| w[0].0 >= w[1].0) {
                instructions_sorted.sort_unstable();
            }
            BlockLookup {
                fixed_definitions,
                maximum_instruction: maximum,
                operand_count,
                implicit_use_units,
                upward_unit_uses,
                unit_kills,
                unit_writer_indices,
                fixed_views,
                instructions_sorted,
                referenced_register_bits,
                slot_summary,
            }
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
    entry.lookup.take();
    &mut entry.block
}
fn block_of(entry: &Arc<BlockEntry>) -> &SelectedBlock {
    &entry.block
}
impl SelectedBlocks {
    /// A mechanical negative filter for split admission. Positive candidates
    /// still undergo the complete source/constraint/use audit at its owner.
    pub fn has_fixed_definition(&self, register: VirtualRegisterId) -> bool {
        self.0.iter().any(|entry| {
            let defs = &entry.lookup().fixed_definitions;
            defs.first()
                .is_some_and(|&first| register >= first && register <= *defs.last().unwrap())
                && defs.binary_search(&register).is_ok()
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
    /// Block index containing the body instruction `instruction`, using each
    /// block's memoized instruction index.
    pub fn instruction_block(&self, instruction: SelectedInstructionId) -> Option<usize> {
        self.0.iter().position(|entry| {
            entry
                .lookup()
                .instructions_sorted
                .binary_search_by_key(&instruction, |&(id, _)| id)
                .is_ok()
        })
    }
    /// Instruction index inside `block_index` for body instruction `instruction`.
    pub fn instruction_index_in_block(
        &self,
        block_index: usize,
        instruction: SelectedInstructionId,
    ) -> Option<usize> {
        let sorted = &self.0.get(block_index)?.lookup().instructions_sorted;
        let mut pos = sorted
            .binary_search_by_key(&instruction, |&(id, _)| id)
            .ok()?;
        while pos > 0 && sorted[pos - 1].0 == instruction {
            pos -= 1;
        }
        Some(sorted[pos].1)
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
        Some(&self.0.get(block_index)?.lookup().slot_summary)
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
