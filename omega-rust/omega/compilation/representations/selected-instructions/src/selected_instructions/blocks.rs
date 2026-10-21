//! Ordered selected blocks in a typed arena, sharing immutable block contents.
//!
//! Recovery edits a few blocks of one function. Cloning that function must not
//! duplicate every instruction and operand vector in unrelated blocks, nor hash
//! them again. The arena owns the block roster; its span preserves block order.
//! Shared entries retain content and its derived digest together. Every mutable
//! escape detaches that entry and clears its digest. Handles and allocation
//! addresses never enter semantic equality or canonical identity.

use super::plan_identity::selected_block_digest;
use super::{SelectedBlock, SelectedInstructionId, VirtualRegisterId};
use arena::{Arena, HandleSpan};
use std::fmt;
use std::ops::{Index, IndexMut};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Default)]
pub struct SelectedBlocks {
    entries: Arena<BlockStorage>,
    order: HandleSpan<BlockStorage>,
}

// Vacant is the arena's zero-handle sentinel; occupied roster rows are always
// Shared. Construction and the arena stay private so it cannot become a block.
#[derive(Clone, Default)]
pub enum BlockStorage {
    #[default]
    Vacant,
    Shared(Arc<BlockEntry>),
}

#[derive(Clone)]
pub struct BlockEntry {
    block: SelectedBlock,
    digest: OnceLock<[u8; 32]>,
    lookup: OnceLock<BlockLookup>,
}
#[derive(Clone)]
struct BlockLookup {
    fixed_definitions: Vec<VirtualRegisterId>,
    maximum_instruction: SelectedInstructionId,
}
impl BlockEntry {
    fn lookup(&self) -> &BlockLookup {
        self.lookup.get_or_init(|| {
            let mut fixed_definitions = Vec::new();
            let mut maximum = super::terminator_instruction(&self.block.terminator).id;
            for instruction in &self.block.instructions {
                maximum = maximum.max(instruction.id);
                fixed_definitions.extend(
                    instruction
                        .operands
                        .iter()
                        .filter(|operand| {
                            operand.fixed_view.is_some()
                                && operand.access != register_model::RegisterOperandAccess::Use
                        })
                        .map(|operand| operand.virtual_register),
                );
            }
            fixed_definitions.sort_unstable();
            fixed_definitions.dedup();
            BlockLookup {
                fixed_definitions,
                maximum_instruction: maximum,
            }
        })
    }
}
impl BlockStorage {
    fn new(block: SelectedBlock) -> Self {
        Self::Shared(Arc::new(BlockEntry {
            block,
            digest: OnceLock::new(),
            lookup: OnceLock::new(),
        }))
    }
    fn entry(&self) -> &Arc<BlockEntry> {
        match self {
            Self::Shared(entry) => entry,
            Self::Vacant => unreachable!("occupied block roster row"),
        }
    }
    fn block(&self) -> &SelectedBlock {
        &self.entry().block
    }
    fn detach(&mut self) -> &mut SelectedBlock {
        let Self::Shared(entry) = self else {
            unreachable!("occupied block roster row")
        };
        let entry = Arc::make_mut(entry);
        entry.digest.take();
        entry.lookup.take();
        &mut entry.block
    }
}
impl SelectedBlocks {
    fn rows(&self) -> &[BlockStorage] {
        self.entries.span_or_empty(self.order)
    }
    fn rows_mut(&mut self) -> &mut [BlockStorage] {
        self.entries.span_mut_or_empty(self.order)
    }
    /// A mechanical negative filter for split admission. Positive candidates
    /// still undergo the complete source/constraint/use audit at its owner.
    pub fn has_fixed_definition(&self, register: VirtualRegisterId) -> bool {
        self.rows().iter().any(|row| {
            row.entry()
                .lookup()
                .fixed_definitions
                .binary_search(&register)
                .is_ok()
        })
    }
    pub fn maximum_instruction_id(&self) -> Option<SelectedInstructionId> {
        self.rows()
            .iter()
            .map(|row| row.entry().lookup().maximum_instruction)
            .max()
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&SelectedBlock> {
        self.rows().get(index).map(BlockStorage::block)
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut SelectedBlock> {
        self.rows_mut().get_mut(index).map(BlockStorage::detach)
    }
    pub fn first(&self) -> Option<&SelectedBlock> {
        self.get(0)
    }
    pub fn last(&self) -> Option<&SelectedBlock> {
        self.rows().last().map(BlockStorage::block)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SelectedBlock> + DoubleEndedIterator {
        self.rows().iter().map(BlockStorage::block)
    }
    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = &mut SelectedBlock> + DoubleEndedIterator {
        self.rows_mut().iter_mut().map(BlockStorage::detach)
    }
    pub fn push(&mut self, block: SelectedBlock) {
        self.entries
            .append_to_span(&mut self.order, BlockStorage::new(block));
    }
    pub fn extend(&mut self, blocks: impl IntoIterator<Item = SelectedBlock>) {
        for block in blocks {
            self.push(block);
        }
    }
    pub fn truncate(&mut self, length: usize) {
        if length >= self.len() {
            return;
        }
        let mut entries = Arena::with_capacity(length);
        let order = entries.insert_many(self.rows().iter().take(length).cloned());
        *self = Self { entries, order };
    }
    pub fn shares_block_storage(&self, other: &Self, index: usize) -> bool {
        self.rows()
            .get(index)
            .zip(other.rows().get(index))
            .is_some_and(|(left, right)| Arc::ptr_eq(left.entry(), right.entry()))
    }
    pub(crate) fn identity_digests(&self) -> impl ExactSizeIterator<Item = [u8; 32]> + '_ {
        self.rows().iter().map(|row| {
            let entry = row.entry();
            *entry
                .digest
                .get_or_init(|| selected_block_digest(&entry.block))
        })
    }
}
impl fmt::Debug for SelectedBlocks {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}
impl PartialEq for SelectedBlocks {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self.rows().iter().zip(other.rows()).all(|(left, right)| {
                Arc::ptr_eq(left.entry(), right.entry()) || left.block() == right.block()
            })
    }
}
impl Eq for SelectedBlocks {}
impl From<Vec<SelectedBlock>> for SelectedBlocks {
    fn from(blocks: Vec<SelectedBlock>) -> Self {
        blocks.into_iter().collect()
    }
}
impl FromIterator<SelectedBlock> for SelectedBlocks {
    fn from_iter<T: IntoIterator<Item = SelectedBlock>>(blocks: T) -> Self {
        let mut entries = Arena::new();
        let order = entries.insert_many(blocks.into_iter().map(BlockStorage::new));
        Self { entries, order }
    }
}
impl Index<usize> for SelectedBlocks {
    type Output = SelectedBlock;
    fn index(&self, index: usize) -> &Self::Output {
        self.rows()[index].block()
    }
}
impl IndexMut<usize> for SelectedBlocks {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.rows_mut()[index].detach()
    }
}
impl<'storage> IntoIterator for &'storage SelectedBlocks {
    type Item = &'storage SelectedBlock;
    type IntoIter = std::iter::Map<
        std::slice::Iter<'storage, BlockStorage>,
        fn(&'storage BlockStorage) -> &'storage SelectedBlock,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.rows().iter().map(BlockStorage::block)
    }
}
impl<'storage> IntoIterator for &'storage mut SelectedBlocks {
    type Item = &'storage mut SelectedBlock;
    type IntoIter = std::iter::Map<
        std::slice::IterMut<'storage, BlockStorage>,
        fn(&'storage mut BlockStorage) -> &'storage mut SelectedBlock,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.rows_mut().iter_mut().map(BlockStorage::detach)
    }
}
