//! Append-only child batches beneath frozen hierarchy nodes.
//!
//! A dense parent index links arena-owned spans. Appending another batch never
//! copies earlier children or relocates a declaration's original child range.

use crate::{Symbol, SymbolHandle};
use arena::{Arena, Handle, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ChildBatch {
    children: HandleSpan<Symbol>,
    next: Handle<ChildBatch>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ChildChain {
    first: Handle<ChildBatch>,
    last: Handle<ChildBatch>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SupplementalChildren {
    batches: Arena<ChildBatch>,
    parents: Vec<ChildChain>,
}

impl SupplementalChildren {
    pub(super) fn append(&mut self, parent: SymbolHandle, children: HandleSpan<Symbol>) {
        if children.is_empty() {
            return;
        }
        let parent_index = parent.arena_index() as usize;
        if self.parents.len() <= parent_index {
            self.parents.resize(parent_index + 1, ChildChain::default());
        }
        let next = self.batches.insert(ChildBatch {
            children,
            next: Handle::invalid(),
        });
        let chain = &mut self.parents[parent_index];
        if chain.last.is_valid() {
            self.batches.get_mut(chain.last).next = next;
        } else {
            chain.first = next;
        }
        chain.last = next;
    }

    pub(super) fn children(&self, parent: SymbolHandle) -> SupplementalChildHandles<'_> {
        SupplementalChildHandles {
            batches: &self.batches,
            next_batch: self
                .parents
                .get(parent.arena_index() as usize)
                .map(|chain| chain.first)
                .unwrap_or_default(),
            children: HandleSpan::empty(),
        }
    }
}

pub(super) struct SupplementalChildHandles<'table> {
    batches: &'table Arena<ChildBatch>,
    next_batch: Handle<ChildBatch>,
    children: HandleSpan<Symbol>,
}

impl Iterator for SupplementalChildHandles<'_> {
    type Item = SymbolHandle;

    fn next(&mut self) -> Option<Self::Item> {
        self.nth(0)
    }

    fn nth(&mut self, mut ordinal: usize) -> Option<Self::Item> {
        loop {
            if ordinal < self.children.len() {
                let start = self.children.start();
                let selected = SymbolHandle::from_parts(
                    start.arena_index() + ordinal as u32,
                    start.generation(),
                );
                let remaining = self.children.count() - ordinal as u32 - 1;
                self.children = if remaining == 0 {
                    HandleSpan::empty()
                } else {
                    HandleSpan::from_parts(
                        SymbolHandle::from_parts(selected.arena_index() + 1, selected.generation()),
                        remaining,
                    )
                };
                return Some(selected);
            }
            ordinal -= self.children.len();
            self.children = HandleSpan::empty();
            if !self.next_batch.is_valid() {
                return None;
            }
            let batch = self.batches.get(self.next_batch);
            self.next_batch = batch.next;
            self.children = batch.children;
        }
    }
}
