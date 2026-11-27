//! Join alternatives at their next common IR coordinate. Inside an acyclic
//! machine reverse postorder visits every predecessor before its successor.
//! A suspended caller resumes only after its selected callee returns.

use super::super::facts::Facts;
use super::activation::Frame;
use super::storage::Storage;
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Reaching a common successor from every arm restores its predecessor's
/// coverage, including a sum with more than two admitted cases.
#[derive(Clone, Default)]
pub(super) enum Coverage {
    #[default]
    All,
    Arms {
        fork: Rc<(Coverage, usize)>,
        returned: Vec<usize>,
    },
    Partial,
}

impl Coverage {
    fn join(&mut self, other: Self) {
        match (&mut *self, other) {
            (Self::All, _) | (_, Self::All) => *self = Self::All,
            (
                Self::Arms { fork, returned },
                Self::Arms {
                    fork: incoming,
                    returned: more,
                },
            ) if Rc::ptr_eq(fork, &incoming) => {
                for arm in more {
                    if let Err(at) = returned.binary_search(&arm) {
                        returned.insert(at, arm);
                    }
                }
                if returned.len() == fork.1 {
                    *self = fork.0.clone();
                }
            }
            _ => *self = Self::Partial,
        }
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

    pub fn fork(self, arms: &[usize], mut queue: impl FnMut(Self)) {
        let coverage = Rc::new((self.coverage.clone(), arms.len()));
        for &arm in arms {
            let mut selected = self.clone();
            selected.coverage = Coverage::Arms {
                fork: coverage.clone(),
                returned: vec![arm],
            };
            selected
                .stack
                .last_mut()
                .expect("a fork has a frame")
                .selected_arm = Some(arm);
            queue(selected);
        }
    }

    fn join(&mut self, other: Self) {
        let previous = self.memory.clone();
        let mut arrivals = super::storage::Arrivals::new(&previous, &other.memory);
        // An owned block formal gets fresh storage on each arrival. At its
        // entry no operation could have taken a reference to that fresh
        // home. Join its contents even though the two allocations differ.
        let mut formals = Vec::new();
        if let (Some(active), Some(incoming)) = (self.stack.last(), other.stack.last())
            && active.position == 0
            && active.cleanup_cursor == 0
            && active.selected_arm.is_none()
        {
            for formal in &active.block.structural_parameters {
                if formal.access == terminal_psi::StructuralAccess::Owned
                    && let Some(left) = active.bindings.places.get(&formal.place)
                    && let Some(right) = incoming.bindings.places.get(&formal.place)
                    && left != right
                {
                    let mut value = self.memory.get(left);
                    arrivals.value(&mut value, &other.memory.get(right));
                    formals.push((formal.place, value));
                }
            }
        }
        for (frame, incoming) in self.stack.iter_mut().zip(other.stack) {
            frame.bindings.join(&incoming.bindings, &mut arrivals);
            if let (Some(captured), Some(other)) = (&mut frame.arrival, incoming.arrival) {
                captured.bindings.join(&other.bindings, &mut arrivals);
            }
        }
        self.memory.join(&other.memory, arrivals);
        if let Some(active) = self.stack.last_mut() {
            for (formal, value) in formals {
                active
                    .bindings
                    .places
                    .insert(formal, self.memory.allocate(value));
            }
        }
        self.coverage.join(other.coverage);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct FramePosition {
    machine: MachineId,
    block_rank: usize,
    operation: usize,
    cleanup: usize,
    arm: Option<usize>,
    arrival: Option<EdgeId>,
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
                arrival: frame.arrival.as_ref().map(|arrival| arrival.edge),
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
