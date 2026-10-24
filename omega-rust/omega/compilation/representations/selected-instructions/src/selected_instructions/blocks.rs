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
#[derive(Clone)]
struct BlockLookup {
    fixed_definitions: Vec<VirtualRegisterId>,
    maximum_instruction: SelectedInstructionId,
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
            entry
                .lookup()
                .fixed_definitions
                .binary_search(&register)
                .is_ok()
        })
    }
    pub fn maximum_instruction_id(&self) -> Option<SelectedInstructionId> {
        self.0
            .iter()
            .map(|entry| entry.lookup().maximum_instruction)
            .max()
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
