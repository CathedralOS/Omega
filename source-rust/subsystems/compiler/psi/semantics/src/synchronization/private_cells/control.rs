//! Join alternatives at their next common IR coordinate. Inside an acyclic
//! machine reverse postorder visits every predecessor before its successor.
//! A suspended caller resumes only after its selected callee returns.

use super::super::facts::Facts;
use super::activation::Frame;
use super::storage::Storage;
use semantic_vocabulary::{BlockId, MachineId};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Returning both arms of the same conditional restores the reachability of
/// their common predecessor. Unrelated incomplete paths carry no such proof.
#[derive(Clone, Default)]
pub(super) enum Coverage {
    #[default]
    All,
    Arm {
        parent: Rc<Coverage>,
        taken: bool,
    },
    Partial,
}

impl Coverage {
    fn join(&mut self, other: Self) {
        *self = match (&*self, other) {
            (Self::All, _) | (_, Self::All) => Self::All,
            (
                Self::Arm { parent, taken },
                Self::Arm {
                    parent: incoming,
                    taken: opposite,
                },
            ) if Rc::ptr_eq(parent, &incoming) && *taken != opposite => (**parent).clone(),
            _ => Self::Partial,
        };
    }
}

#[derive(Clone)]
pub(super) struct Execution<'m> {
    pub stack: Vec<Frame<'m>>,
    pub memory: Storage,
    pub coverage: Coverage,
}

impl Execution<'_> {
    pub fn certain(&self) -> bool {
        matches!(self.coverage, Coverage::All)
    }

    pub fn fork(&mut self) -> Option<Self> {
        let parent = Rc::new(std::mem::take(&mut self.coverage));
        let mut other = self.clone();
        self.coverage = Coverage::Arm {
            parent: parent.clone(),
            taken: true,
        };
        other.coverage = Coverage::Arm {
            parent,
            taken: false,
        };
        self.stack.last_mut()?.selected_arm = Some(true);
        other.stack.last_mut()?.selected_arm = Some(false);
        Some(other)
    }

    fn join(&mut self, other: Self) {
        for (frame, incoming) in self.stack.iter_mut().zip(other.stack) {
            frame
                .bindings
                .values
                .retain_equal(&incoming.bindings.values);
            frame
                .bindings
                .places
                .retain_equal(&incoming.bindings.places);
        }
        self.memory.join(&other.memory);
        self.coverage.join(other.coverage);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct FramePosition {
    machine: MachineId,
    block_rank: usize,
    operation: usize,
    cleanup: usize,
    arm: Option<bool>,
}

#[derive(PartialEq, Eq)]
struct Coordinate(Vec<FramePosition>);

impl Ord for Coordinate {
    fn cmp(&self, other: &Self) -> Ordering {
        for (a, b) in self.0.iter().zip(&other.0) {
            let order = a.cmp(b);
            if !order.is_eq() {
                return order;
            }
        }
        other.0.len().cmp(&self.0.len())
    }
}

impl PartialOrd for Coordinate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub(super) struct Frontier<'m> {
    linear: Option<Execution<'m>>,
    ranks: Facts<MachineId, Facts<BlockId, usize>>,
    pending: BTreeMap<Coordinate, Execution<'m>>,
    visited: BTreeSet<Coordinate>,
}

impl<'m> Frontier<'m> {
    pub fn queue(&mut self, path: Execution<'m>) {
        if path.stack.is_empty() {
            return;
        }
        if path.certain() && self.pending.is_empty() {
            self.linear = Some(path);
            return;
        }
        let mut coordinates = Vec::with_capacity(path.stack.len());
        for frame in &path.stack {
            let ranks = self.ranks.get_or_insert_with(frame.machine.id, || {
                crate::control_graph::reverse_postorder(frame.machine)
                    .into_iter()
                    .enumerate()
                    .map(|(rank, block)| (block, rank))
                    .collect()
            });
            let Some(&block_rank) = ranks.get(&frame.block.id) else {
                return;
            };
            coordinates.push(FramePosition {
                machine: frame.machine.id,
                block_rank,
                operation: frame.position,
                cleanup: frame.cleanup_cursor,
                arm: frame.selected_arm,
            });
        }
        let key = Coordinate(coordinates);
        if self.visited.contains(&key) {
            return;
        }
        match self.pending.entry(key) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(path);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => slot.get_mut().join(path),
        }
    }

    pub fn next(&mut self) -> Option<Execution<'m>> {
        if self.linear.is_some() {
            return self.linear.take();
        }
        let (key, path) = self.pending.pop_first()?;
        self.visited.insert(key);
        Some(path)
    }
}
