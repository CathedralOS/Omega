//! Universal resource replay of one checked activation's guarded path program.
//!
//! The canonical owner must derive every node, outcome guard, call summary and
//! retained return value before constructing this program. It is not an input
//! certificate or permission table. This replay checks every incoming ledger;
//! one successful path or one balanced origin witness cannot close another.

use std::collections::{BTreeSet, VecDeque};

use super::windows_handle_lifetimes::{
    HandleInstance, HandleLifetimeFailure, HandleLifetimes, NativeOccurrence, ScalarOccurrence,
};

pub(super) enum HandleTransition {
    Acquire {
        instance: HandleInstance,
        occurrence: NativeOccurrence,
        flags: u32,
    },
    Release {
        instance: HandleInstance,
        occurrence: NativeOccurrence,
    },
}

pub(super) struct HandleReturn {
    pub occurrence: ScalarOccurrence,
    pub retained: Vec<HandleInstance>,
}

/// These are analysis-owned nodes, including separately checked native outcome
/// branches. They do not fabricate Terminal blocks or operations. The source
/// owner retains their complete canonical position/guard correspondence.
pub(super) struct HandlePathNode {
    pub transitions: Vec<HandleTransition>,
    pub successors: Vec<usize>,
    pub returning: Option<HandleReturn>,
}

pub(super) struct HandlePathProgram {
    pub entry: usize,
    pub nodes: Vec<HandlePathNode>,
    /// Exact ordinary-release occurrences this scope proposes to classify.
    /// Unvisited candidates and an empty roster cannot provide release proof.
    pub required_releases: BTreeSet<NativeOccurrence>,
}

#[derive(Debug)]
pub(super) enum HandlePathFailure {
    InvalidProgram,
    Lifetime {
        node: usize,
        reason: HandleLifetimeFailure,
    },
    UncoveredRelease,
    AnalysisBudget,
}

pub(super) struct HandlePathProof {
    pub releases: BTreeSet<NativeOccurrence>,
    pub visited_nodes: BTreeSet<usize>,
    pub returning_paths: usize,
}

impl HandlePathProgram {
    pub(super) fn replay(
        &self,
        incoming: HandleLifetimes,
    ) -> Result<HandlePathProof, HandlePathFailure> {
        if self.nodes.get(self.entry).is_none() {
            // Refused: ordinary-release scope has no entry.
            return Err(HandlePathFailure::InvalidProgram);
        }
        if self.required_releases.is_empty() {
            // Refused: ordinary-release scope has no actual release candidate.
            return Err(HandlePathFailure::InvalidProgram);
        }
        for node in &self.nodes {
            if node
                .successors
                .iter()
                .any(|target| *target >= self.nodes.len())
            {
                // Refused: ordinary-release scope has an absent successor.
                return Err(HandlePathFailure::InvalidProgram);
            }
            if node.returning.is_some() && !node.successors.is_empty() {
                // Refused: ordinary-release return still has a continuation.
                return Err(HandlePathFailure::InvalidProgram);
            }
            if node.returning.is_none() && node.successors.is_empty() {
                // Refused: ordinary-release path falls off without a proved completion.
                return Err(HandlePathFailure::InvalidProgram);
            }
            for transition in &node.transitions {
                if let HandleTransition::Release { occurrence, .. } = transition
                    && !self.required_releases.contains(occurrence)
                {
                    // Refused: ordinary-release scope substitutes its candidate roster.
                    return Err(HandlePathFailure::InvalidProgram);
                }
            }
        }
        let mut seen = vec![Vec::<HandleLifetimes>::new(); self.nodes.len()];
        seen[self.entry].push(incoming.clone());
        let mut pending = VecDeque::from([(self.entry, incoming)]);
        let mut discovered_states = 1usize;
        let mut proof = HandlePathProof {
            releases: BTreeSet::new(),
            visited_nodes: BTreeSet::new(),
            returning_paths: 0,
        };
        while let Some((index, mut ledger)) = pending.pop_front() {
            let node = &self.nodes[index];
            proof.visited_nodes.insert(index);
            for transition in &node.transitions {
                transition
                    .apply(&mut ledger)
                    .map_err(|reason| HandlePathFailure::Lifetime {
                        node: index,
                        reason,
                    })?;
                if let HandleTransition::Release { occurrence, .. } = transition {
                    proof.releases.insert(*occurrence);
                }
            }
            if let Some(returned) = &node.returning {
                ledger
                    .returning_path(returned.occurrence, &returned.retained)
                    .map_err(|reason| HandlePathFailure::Lifetime {
                        node: index,
                        reason,
                    })?;
                proof.returning_paths += 1;
            }
            for &target in &node.successors {
                if seen[target].contains(&ledger) {
                    continue;
                }
                if discovered_states >= 65_536 {
                    return Err(HandlePathFailure::AnalysisBudget);
                }
                discovered_states += 1;
                seen[target].push(ledger.clone());
                pending.push_back((target, ledger.clone()));
            }
        }
        if proof.returning_paths == 0 {
            // Refused: ordinary-release scope has no actual returning path.
            return Err(HandlePathFailure::InvalidProgram);
        }
        if proof.visited_nodes.len() != self.nodes.len() {
            // Refused: ordinary-release scope contains unproved unreachable nodes.
            return Err(HandlePathFailure::InvalidProgram);
        }
        if !self.required_releases.is_subset(&proof.releases) {
            return Err(HandlePathFailure::UncoveredRelease);
        }
        Ok(proof)
    }
}

impl HandleTransition {
    fn apply(&self, ledger: &mut HandleLifetimes) -> Result<(), HandleLifetimeFailure> {
        match *self {
            Self::Acquire {
                instance,
                occurrence,
                flags,
            } => ledger.acquire_file(instance, occurrence, flags),
            Self::Release {
                instance,
                occurrence,
            } => ledger.release_file(instance, occurrence),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::windows_handle_lifetimes::ScalarUseSite;
    use super::*;
    use semantic_vocabulary::{BoundaryMachineId, EdgeId, InvocationSite, MachineId, OperationId};

    fn native(operation: u64) -> NativeOccurrence {
        NativeOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: InvocationSite::Operation(OperationId::new(operation).unwrap()),
            context: [1; 32],
            boundary: BoundaryMachineId::new(1).unwrap(),
        }
    }

    fn returned(edge: u64) -> HandleReturn {
        HandleReturn {
            occurrence: ScalarOccurrence {
                machine: MachineId::new(1).unwrap(),
                site: ScalarUseSite::Edge(EdgeId::new(edge).unwrap()),
                context: [1; 32],
            },
            retained: Vec::new(),
        }
    }

    #[test]
    fn one_closed_return_cannot_cover_another_returning_path() {
        let instance = HandleInstance(0);
        let close = native(2);
        let mut program = HandlePathProgram {
            entry: 0,
            required_releases: BTreeSet::from([close]),
            nodes: vec![
                HandlePathNode {
                    transitions: vec![HandleTransition::Acquire {
                        instance,
                        occurrence: native(1),
                        flags: 0,
                    }],
                    successors: vec![1, 2],
                    returning: None,
                },
                HandlePathNode {
                    transitions: vec![HandleTransition::Release {
                        instance,
                        occurrence: close,
                    }],
                    successors: Vec::new(),
                    returning: Some(returned(1)),
                },
                HandlePathNode {
                    transitions: Vec::new(),
                    successors: Vec::new(),
                    returning: Some(returned(2)),
                },
            ],
        };
        assert!(matches!(
            program.replay(HandleLifetimes::default()),
            Err(HandlePathFailure::Lifetime {
                node: 2,
                reason: HandleLifetimeFailure::MissingRelease { .. }
            })
        ));
        let second = native(3);
        program.required_releases.insert(second);
        program.nodes[2]
            .transitions
            .push(HandleTransition::Release {
                instance,
                occurrence: second,
            });
        let proof = program.replay(HandleLifetimes::default()).unwrap();
        assert_eq!(proof.returning_paths, 2);
        assert_eq!(proof.releases, BTreeSet::from([close, second]));
    }
}
