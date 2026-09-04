//! Query-local budget, memo, and result vocabulary.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{IntegerValue, ScalarTerm};
use psi_proof_admission::{ProofNode, ProofRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SearchBudget {
    pub(super) definition_visits: usize,
    pub(super) depth: usize,
    pub(super) computed_joins: usize,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            definition_visits: 128,
            depth: 32,
            computed_joins: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SearchUsage {
    pub(super) definition_visits: usize,
    pub(super) peak_depth: usize,
    pub(super) memo_hits: usize,
    pub(super) computed_joins: usize,
}

#[derive(Debug, Clone)]
pub(super) struct SearchOutcome {
    pub(super) proof: Option<ProofNode>,
    pub(super) usage: SearchUsage,
    pub(super) exhausted: bool,
}

#[derive(Debug, Clone)]
pub(super) struct EndpointProof {
    pub(super) value: IntegerValue,
    pub(super) proof: ProofNode,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Query {
    operand: ScalarTerm,
    lower: bool,
    cutoff: usize,
}

impl Query {
    pub(super) fn new(operand: &ScalarTerm, lower: bool, cutoff: usize) -> Self {
        Self {
            operand: operand.clone(),
            lower,
            cutoff,
        }
    }
}

pub(super) struct SearchState {
    budget: SearchBudget,
    usage: SearchUsage,
    exhausted: bool,
    active: BTreeSet<Query>,
    memo: BTreeMap<Query, Option<EndpointProof>>,
}

impl SearchState {
    pub(super) fn new(budget: SearchBudget) -> Self {
        Self {
            budget,
            usage: SearchUsage::default(),
            exhausted: false,
            active: BTreeSet::new(),
            memo: BTreeMap::new(),
        }
    }

    pub(super) fn enter(&mut self, query: &Query, depth: usize) -> bool {
        if depth > self.budget.depth {
            self.exhausted = true;
            return false;
        }
        self.usage.peak_depth = self.usage.peak_depth.max(depth);
        self.active.insert(query.clone())
    }

    pub(super) fn leave(&mut self, query: Query, result: Option<EndpointProof>) {
        self.active.remove(&query);
        if !self.exhausted {
            self.memo.insert(query, result);
        }
    }

    pub(super) fn memoized(&mut self, query: &Query) -> Option<Option<EndpointProof>> {
        let result = self.memo.get(query).cloned();
        if result.is_some() {
            self.usage.memo_hits += 1;
        }
        result
    }

    pub(super) fn visit_definition(&mut self) -> bool {
        if self.usage.definition_visits >= self.budget.definition_visits {
            self.exhausted = true;
            return false;
        }
        self.usage.definition_visits += 1;
        true
    }

    pub(super) fn visit_computed_join(&mut self) -> bool {
        if self.usage.computed_joins >= self.budget.computed_joins {
            self.exhausted = true;
            return false;
        }
        self.usage.computed_joins += 1;
        true
    }

    pub(super) fn exhausted(&self) -> bool {
        self.exhausted
    }

    pub(super) fn finish(self, proof: Option<ProofNode>) -> SearchOutcome {
        SearchOutcome {
            proof: (!self.exhausted).then_some(proof).flatten(),
            usage: self.usage,
            exhausted: self.exhausted,
        }
    }
}

pub(super) fn cited_proof(
    proposition: &psi_core::Proposition,
    assumption: Option<usize>,
    axiom: Option<usize>,
) -> ProofNode {
    ProofNode {
        conclusion: proposition.clone(),
        rule: match (assumption, axiom) {
            (Some(index), None) => ProofRule::Assumption { index },
            (None, Some(index)) => ProofRule::SemanticAxiom { index },
            _ => unreachable!("one citation origin is present"),
        },
    }
}
