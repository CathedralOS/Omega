//! Which restoration windows are open when a call parks?
//!
//! A `BorrowedMove` vacates a place reached through an exclusive loan, and a
//! later `StructuralStore` to that same captured place re-establishes it
//! (`wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration`). A
//! suspension retains every window still open, so the crossing names each one.
//! This pass only follows the operations. When it cannot tell which windows are
//! open, because arrivals disagree or a store's place is not provably the same
//! as or disjoint from an open window, the crossing refuses rather than guess.
//! Whether a window is lawful is the checker's judgement.
use super::super::loans::captured_place;
use super::super::view_loans::{Relation, SelectorPremise, relate};
use crate::{BlockTerminator, BranchBlock, MachineBody, OperationKind};

/// Operation indices of the open `BorrowedMove`s, in the order they ran.
#[derive(Clone, Debug, Eq, PartialEq)]
enum State {
    Known(Vec<usize>),
    Unknown,
}

pub(super) struct Windows<'a> {
    body: &'a MachineBody,
    premises: &'a [SelectorPremise],
    entry: Vec<Option<State>>,
}

impl<'a> Windows<'a> {
    pub(super) fn new(
        body: &'a MachineBody,
        premises: &'a [SelectorPremise],
    ) -> Self {
        let mut windows = Self {
            body,
            premises,
            entry: vec![None; body.blocks.len()],
        };
        if body.blocks.is_empty() {
            return windows;
        }
        windows.entry[0] = Some(State::Known(Vec::new()));
        let mut pending = vec![0];
        while let Some(index) = pending.pop() {
            let block = &body.blocks[index];
            let exit = windows.advance(windows.entry[index].clone().expect("a queued block has an entry"), block, block.operations.end);
            let targets = match &block.terminator {
                BlockTerminator::Jump(edge) => vec![edge.target],
                BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true.target, when_false.target],
                _ => Vec::new(),
            };
            // A block's entry goes from unvisited to its first arrival's state,
            // and to unknown when a later arrival disagrees. It never changes
            // back, so the loop stops.
            for target in targets {
                let slot = &mut windows.entry[target.get() - 1];
                let joined = match slot {
                    None => exit.clone(),
                    Some(previous) if *previous == exit => continue,
                    Some(_) => State::Unknown,
                };
                if slot.as_ref() != Some(&joined) {
                    *slot = Some(joined);
                    pending.push(target.get() - 1);
                }
            }
        }
        windows
    }

    /// Apply this block's operations before `end` to the windows open at its entry.
    fn advance(
        &self,
        mut state: State,
        block: &BranchBlock,
        end: usize,
    ) -> State {
        for index in block.operations.start..end {
            let State::Known(open) = &mut state else { break };
            match &self.body.operations[index].kind {
                OperationKind::BorrowedMove { .. } => open.push(index),
                OperationKind::StructuralStore { destination, .. } => {
                    let target = captured_place(self.body, destination);
                    let mut unknown = false;
                    open.retain(|&moved| {
                        let OperationKind::BorrowedMove { source } = &self.body.operations[moved].kind else {
                            unreachable!("open windows are moves")
                        };
                        match relate(self.body, self.premises, &captured_place(self.body, source), &target) {
                            Relation::Same => false,
                            Relation::Disjoint => true,
                            Relation::Overlap | Relation::Unproved => {
                                unknown = true;
                                true
                            }
                        }
                    });
                    if unknown {
                        state = State::Unknown;
                    }
                }
                _ => {}
            }
        }
        state
    }

    /// The windows open just before operation `end` of `block`, or at its
    /// terminator when `end` is the block's end. `None` when a crossing there
    /// cannot name them.
    pub(super) fn open_before(
        &self,
        block: &BranchBlock,
        end: usize,
    ) -> Option<Vec<crate::OpenWindow>> {
        let entry = self.entry[block.id.get() - 1].clone()?;
        let State::Known(open) = self.advance(entry, block, end) else {
            return None;
        };
        Some(
            open.into_iter()
                .map(|index| {
                    let OperationKind::BorrowedMove { source } = &self.body.operations[index].kind else {
                        unreachable!("open windows are moves")
                    };
                    crate::OpenWindow {
                        opened: self.body.operations[index].id,
                        place: captured_place(self.body, source),
                    }
                })
                .collect(),
        )
    }
}
