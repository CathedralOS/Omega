//! Match origin transport through the same actual invocation on both sides.
//!
//! An ordinary return enters one callee's formal namespace; reading a formal
//! leaves that namespace only through the matching caller. Computing balanced
//! paths as a finite relation handles recursive equations without unrolling a
//! call stack or treating a revisited machine as preserving. This relation is
//! custody transport, not scalar equality, initialization or lifetime proof.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use semantic_vocabulary::MachineId;

use super::windows_handle_calls::Caller;
use super::windows_handle_origins::{HandleOrigins, OriginEdgeRole, ScalarSubject};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ActivationFrame {
    pub caller: Caller,
    pub callee: MachineId,
}

#[derive(Clone, Debug)]
pub(super) enum BalancedDerivation {
    Identity,
    Local,
    Compose,
    Invocation,
}

/// Analysis-owned equation coordinates, independent of Terminal identities.
/// Each nontrivial pair records the rule that derived it.
pub(super) struct ActivationPaths {
    pub subjects: Vec<ScalarSubject>,
    index: BTreeMap<ScalarSubject, usize>,
    pub derivations: BTreeMap<(usize, usize), BalancedDerivation>,
    outgoing: Vec<BTreeSet<usize>>,
    actuals: Vec<Vec<(ActivationFrame, usize)>>,
    returned: Vec<Vec<InnerContext>>,
}

#[derive(Default)]
struct RelationQueue {
    discovered: BTreeSet<(usize, usize)>,
    pending: VecDeque<((usize, usize), BalancedDerivation)>,
}

impl RelationQueue {
    fn insert(
        &mut self,
        pair: (usize, usize),
        derivation: BalancedDerivation,
    ) -> Result<(), String> {
        if self.discovered.contains(&pair) {
            return Ok(());
        }
        if self.discovered.len() >= 65_536 {
            return Err(
                "native handle activation relation exceeds its finite analysis budget".into(),
            );
        }
        self.discovered.insert(pair);
        self.pending.push_back((pair, derivation));
        Ok(())
    }
}

impl ActivationPaths {
    pub(super) fn derive(origins: &HandleOrigins) -> Result<Self, String> {
        let subjects = origins.equations.keys().cloned().collect::<Vec<_>>();
        let index = subjects
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, subject)| (subject, index))
            .collect::<BTreeMap<_, _>>();
        let mut result = Self {
            outgoing: vec![BTreeSet::new(); subjects.len()],
            actuals: vec![Vec::new(); subjects.len()],
            returned: vec![Vec::new(); subjects.len()],
            subjects,
            index,
            derivations: BTreeMap::new(),
        };
        let mut incoming = vec![BTreeSet::new(); result.subjects.len()];
        let mut returns =
            vec![BTreeMap::<ActivationFrame, Vec<usize>>::new(); result.subjects.len()];
        let mut actual_index =
            vec![BTreeMap::<ActivationFrame, Vec<usize>>::new(); result.subjects.len()];
        let mut pending = RelationQueue::default();
        for (node, subject) in result.subjects.iter().enumerate() {
            pending.insert((node, node), BalancedDerivation::Identity)?;
            let equation = &origins.equations[subject];
            for edge in &equation.sources {
                let source = *result
                    .index
                    .get(&edge.source)
                    .ok_or("activation equation omits a demanded original source")?;
                match edge.role {
                    OriginEdgeRole::Returned { caller, callee } => {
                        returns[source]
                            .entry(ActivationFrame { caller, callee })
                            .or_default()
                            .push(node);
                        result.returned[node].push(InnerContext {
                            frame: ActivationFrame { caller, callee },
                            returned: source,
                        });
                    }
                    OriginEdgeRole::Actual { caller, callee } => {
                        result.actuals[node].push((ActivationFrame { caller, callee }, source));
                        actual_index[node]
                            .entry(ActivationFrame { caller, callee })
                            .or_default()
                            .push(source);
                    }
                    OriginEdgeRole::Operand | OriginEdgeRole::Stored | OriginEdgeRole::Arrival => {
                        pending.insert((node, source), BalancedDerivation::Local)?;
                    }
                }
            }
        }
        while let Some(((left, right), derivation)) = pending.pending.pop_front() {
            // Prepare only associations touching this newly discovered pair.
            // Never rescan every graph node or every existing relation pair.
            for &previous in &incoming[left] {
                pending.insert((previous, right), BalancedDerivation::Compose)?;
            }
            for &following in &result.outgoing[right] {
                pending.insert((left, following), BalancedDerivation::Compose)?;
            }
            for (&frame, caller_results) in &returns[left] {
                for &actual_value in actual_index[right].get(&frame).into_iter().flatten() {
                    for &caller_result in caller_results {
                        pending.insert(
                            (caller_result, actual_value),
                            BalancedDerivation::Invocation,
                        )?;
                    }
                }
            }
            incoming[right].insert(left);
            result.outgoing[left].insert(right);
            result.derivations.insert((left, right), derivation);
        }
        Ok(result)
    }

    pub(super) fn subject_index(&self, subject: &ScalarSubject) -> Result<usize, String> {
        self.index
            .get(subject)
            .copied()
            .ok_or("activation query has no original origin equation".into())
    }

    /// Context outside a query's current activation is a finite labelled graph,
    /// rather than an arbitrary caller chosen while inside a returned value.
    /// Each outer arc first follows a balanced path, then exits through exactly
    /// one actual invocation. The lifetime owner must retain that frame label.
    pub(super) fn outer_contexts(
        &self,
        root: usize,
    ) -> Result<BTreeMap<usize, Vec<ActivationFrame>>, String> {
        if root >= self.subjects.len() {
            return Err("activation root is outside its equation inventory".into());
        }
        let mut contexts = BTreeMap::new();
        let mut pending = vec![root];
        while let Some(start) = pending.pop() {
            if contexts.contains_key(&start) {
                continue;
            }
            let mut exits = Vec::new();
            for &formal in &self.outgoing[start] {
                for &(frame, actual) in &self.actuals[formal] {
                    exits.push(frame);
                    pending.push(actual);
                }
            }
            contexts.insert(start, exits);
        }
        Ok(contexts)
    }

    pub(super) fn balanced_sources(&self, subject: usize) -> Result<&BTreeSet<usize>, String> {
        self.outgoing
            .get(subject)
            .ok_or("balanced origin query is outside its equation inventory".into())
    }

    /// A producing native event can occur inside the returned callee, before
    /// any formal is substituted back to the caller. Preserve these unmatched
    /// inward frames too. Balanced reachability alone would omit such births.
    /// The relation is existential transport; it never discharges the owner's
    /// universal obligation to check every original return/path alternative.
    pub(super) fn inner_contexts(
        &self,
        root: usize,
    ) -> Result<BTreeMap<usize, Vec<InnerContext>>, String> {
        if root >= self.subjects.len() {
            return Err("inner activation root is outside its equation inventory".into());
        }
        let mut contexts = BTreeMap::new();
        let mut pending = vec![root];
        while let Some(start) = pending.pop() {
            if contexts.contains_key(&start) {
                continue;
            }
            let mut entries = Vec::new();
            for &result in &self.outgoing[start] {
                for &entry in &self.returned[result] {
                    entries.push(entry);
                    pending.push(entry.returned);
                }
            }
            contexts.insert(start, entries);
        }
        Ok(contexts)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InnerContext {
    pub frame: ActivationFrame,
    pub returned: usize,
}

#[cfg(test)]
mod tests {
    use super::super::windows_handle_calls::CallSite;
    use super::super::windows_handle_origins::{OriginEdge, OriginEquation, OriginKind};
    use super::*;
    use semantic_vocabulary::{BlockId, InvocationSite, OperationId, ValueId};

    fn value(machine: u64, value: u64) -> ScalarSubject {
        ScalarSubject::Value {
            machine: MachineId::new(machine).unwrap(),
            value: ValueId::new(value).unwrap(),
        }
    }

    fn caller(operation: u64) -> Caller {
        Caller {
            machine: MachineId::new(1).unwrap(),
            block: BlockId::new(1).unwrap(),
            operation_index: operation as usize,
            site: CallSite::Invocation(InvocationSite::Operation(
                OperationId::new(operation).unwrap(),
            )),
        }
    }

    #[test]
    fn returned_formal_cannot_take_a_sibling_invocations_actual() {
        let result = value(1, 1);
        let formal = value(2, 1);
        let right_actual = value(1, 2);
        let sibling_actual = value(1, 3);
        let callee = MachineId::new(2).unwrap();
        let returned = OriginEquation {
            kind: OriginKind::Alternatives,
            sources: vec![OriginEdge {
                role: OriginEdgeRole::Returned {
                    caller: caller(1),
                    callee,
                },
                source: formal.clone(),
            }],
        };
        let input = OriginEquation {
            kind: OriginKind::Alternatives,
            sources: vec![
                OriginEdge {
                    role: OriginEdgeRole::Actual {
                        caller: caller(1),
                        callee,
                    },
                    source: right_actual.clone(),
                },
                OriginEdge {
                    role: OriginEdgeRole::Actual {
                        caller: caller(2),
                        callee,
                    },
                    source: sibling_actual.clone(),
                },
            ],
        };
        let origins = HandleOrigins {
            equations: BTreeMap::from([
                (result.clone(), returned),
                (formal.clone(), input),
                (
                    right_actual.clone(),
                    OriginEquation {
                        kind: OriginKind::ExternalInput,
                        sources: Vec::new(),
                    },
                ),
                (
                    sibling_actual.clone(),
                    OriginEquation {
                        kind: OriginKind::ExternalInput,
                        sources: Vec::new(),
                    },
                ),
            ]),
        };
        let paths = ActivationPaths::derive(&origins).unwrap();
        let result = paths.subject_index(&result).unwrap();
        assert!(
            paths
                .balanced_sources(result)
                .unwrap()
                .contains(&paths.subject_index(&right_actual).unwrap())
        );
        let inner = paths.inner_contexts(result).unwrap();
        let [entry] = inner[&result].as_slice() else {
            panic!("the actual return must retain exactly its own inward frame");
        };
        assert_eq!(entry.frame.caller, caller(1));
        assert_eq!(entry.returned, paths.subject_index(&formal).unwrap());
        assert!(paths.outer_contexts(result).unwrap()[&result].is_empty());
        assert!(
            !paths
                .balanced_sources(result)
                .unwrap()
                .contains(&paths.subject_index(&sibling_actual).unwrap())
        );
    }
}
