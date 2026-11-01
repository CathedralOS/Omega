//! Optional reuse of a completed traversal at an identical cyclic arrival.
//!
//! Forwarded values alone do not identify a proof question. A diamond may
//! forward the same scalar under opposite guards, and only the second guard
//! reaches a requested operand. Ancestry remains the recursion boundary; this
//! log only avoids repeating a successfully completed, exact context.

use super::{CitationFacts, PendingStructuralCitation, StructuralJudge, StructuralTerm};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use symbols::SymbolHandle;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ArrivalBucket {
    state: SymbolHandle,
    start_index: usize,
    collect_citations: bool,
    environment: u64,
}

pub(super) struct StructuralArrival<'program> {
    pub(super) state: SymbolHandle,
    pub(super) start_index: usize,
    pub(super) collect_citations: bool,
    pub(super) parent: usize,
    pub(super) judge: StructuralJudge<'program>,
    pub(super) parameter_names: Vec<String>,
    pub(super) environment: Vec<(String, StructuralTerm)>,
    pub(super) hypotheses: Vec<(String, StructuralTerm)>,
    pub(super) equations: Vec<(StructuralTerm, StructuralTerm)>,
    pub(super) citations: Vec<PendingStructuralCitation>,
    pub(super) guarantees: CitationFacts,
    pub(super) path: Vec<SymbolHandle>,
}

impl StructuralArrival<'_> {
    fn bucket(&self) -> ArrivalBucket {
        // This existing semantic hash is only a candidate filter. Exact
        // payload comparison below retains hidden observation and landing
        // metadata even when it is absent from the semantic Debug view.
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for (name, term) in &self.environment {
            name.hash(&mut hash);
            format!("{:?}", term.semantic_value()).hash(&mut hash);
        }
        ArrivalBucket {
            state: self.state,
            start_index: self.start_index,
            collect_citations: self.collect_citations,
            environment: hash.finish(),
        }
    }

    fn same_inputs(&self, other: &Self) -> bool {
        self.judge.same_traversal_inputs(&other.judge)
            && self.path == other.path
            && self.parameter_names == other.parameter_names
            && same_named_values(&self.environment, &other.environment)
            && same_named_values(&self.hypotheses, &other.hypotheses)
            && self.equations.len() == other.equations.len()
            && self.equations.iter().zip(&other.equations).all(
                |((left, right), (other_left, other_right))| {
                    left.same_resolution_payload(other_left)
                        && right.same_resolution_payload(other_right)
                },
            )
            && self.citations.len() == other.citations.len()
            && self.citations.iter().zip(&other.citations).all(
                |((target, arguments), (other_target, other_arguments))| {
                    target.same_resolution_payload(other_target)
                        && arguments.len() == other_arguments.len()
                        && arguments
                            .iter()
                            .zip(other_arguments)
                            .all(|(left, right)| left.same_resolution_payload(right))
                },
            )
            && self.guarantees.same_resolution_payload(&other.guarantees)
    }
}

fn same_named_values(
    left: &[(String, StructuralTerm)],
    right: &[(String, StructuralTerm)],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((name, value), (other_name, other_value))| {
                name == other_name && value.same_resolution_payload(other_value)
            })
}

#[derive(Default)]
pub(in crate::proof_contracts::contract_entailment) struct StructuralArrivalLog<'program> {
    completed: HashMap<ArrivalBucket, Vec<StructuralArrival<'program>>>,
}

impl<'program> StructuralArrivalLog<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn new() -> Self {
        Self::default()
    }

    pub(super) fn contains_completed(&self, arrival: &StructuralArrival<'program>) -> bool {
        self.completed.get(&arrival.bucket()).is_some_and(|bucket| {
            bucket.iter().any(|previous| {
                // Fanout siblings retain their independent deliveries, as in
                // the collecting walk. Only another frame may reuse this work.
                previous.parent != arrival.parent && previous.same_inputs(arrival)
            })
        })
    }

    pub(super) fn complete(&mut self, arrival: StructuralArrival<'program>) {
        let bucket = self.completed.entry(arrival.bucket()).or_default();
        if !bucket
            .iter()
            .any(|previous| previous.parent == arrival.parent && previous.same_inputs(&arrival))
        {
            bucket.push(arrival);
        }
    }
}
