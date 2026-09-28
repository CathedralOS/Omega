//! Recursive direct-add conjunction search under one shared work budget.

use proof_admission::ProofNode;
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use proof_admission::ProofRule;
use semantic_vocabulary::IntegerValue;
use std::collections::{BTreeMap, BTreeSet};

mod compute;
mod definitions;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    let outcome = prove_with_budget(
        context,
        goal,
        integer_type,
        left,
        right,
        target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
        SearchBudget::default(),
    );
    let _usage = outcome.usage;
    (!outcome.exhausted).then_some(outcome.proof).flatten()
}

#[allow(clippy::too_many_arguments)]
fn prove_with_budget(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    budget: SearchBudget,
) -> SearchOutcome {
    let mut state = SearchState::new(budget);
    let proof = compute::prove(
        context,
        goal,
        integer_type,
        left,
        right,
        target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
        &mut state,
    );
    state.finish(proof)
}

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SearchBudget {
    definition_visits: usize,
    depth: usize,
    computed_joins: usize,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            definition_visits: 128,
            depth: 32,
            // Bounded compositions of computed sums: each join nests one
            // `IntegerExactAddDefinitionBound` certificate, so the count tracks
            // how deep add-of-adds chains may stack inside one obligation.
            computed_joins: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SearchUsage {
    definition_visits: usize,
    peak_depth: usize,
    memo_hits: usize,
    computed_joins: usize,
}

#[derive(Debug, Clone)]
struct SearchOutcome {
    proof: Option<ProofNode>,
    usage: SearchUsage,
    exhausted: bool,
}

#[derive(Debug, Clone)]
struct EndpointProof {
    value: IntegerValue,
    proof: ProofNode,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Query {
    operand: ScalarTerm,
    lower: bool,
    cutoff: usize,
}

impl Query {
    fn new(operand: &ScalarTerm, lower: bool, cutoff: usize) -> Self {
        Self {
            operand: operand.clone(),
            lower,
            cutoff,
        }
    }
}

struct SearchState {
    budget: SearchBudget,
    usage: SearchUsage,
    exhausted: bool,
    active: BTreeSet<Query>,
    memo: BTreeMap<Query, Option<EndpointProof>>,
}

impl SearchState {
    fn new(budget: SearchBudget) -> Self {
        Self {
            budget,
            usage: SearchUsage::default(),
            exhausted: false,
            active: BTreeSet::new(),
            memo: BTreeMap::new(),
        }
    }

    fn enter(&mut self, query: &Query, depth: usize) -> bool {
        if depth > self.budget.depth {
            self.exhausted = true;
            return false;
        }
        self.usage.peak_depth = self.usage.peak_depth.max(depth);
        self.active.insert(query.clone())
    }

    fn leave(&mut self, query: Query, result: Option<EndpointProof>) {
        self.active.remove(&query);
        if !self.exhausted {
            self.memo.insert(query, result);
        }
    }

    fn memoized(&mut self, query: &Query) -> Option<Option<EndpointProof>> {
        let result = self.memo.get(query).cloned();
        if result.is_some() {
            self.usage.memo_hits += 1;
        }
        result
    }

    fn visit_definition(&mut self) -> bool {
        if self.usage.definition_visits >= self.budget.definition_visits {
            self.exhausted = true;
            return false;
        }
        self.usage.definition_visits += 1;
        true
    }

    fn visit_computed_join(&mut self) -> bool {
        if self.usage.computed_joins >= self.budget.computed_joins {
            self.exhausted = true;
            return false;
        }
        self.usage.computed_joins += 1;
        true
    }

    fn exhausted(&self) -> bool {
        self.exhausted
    }

    fn finish(self, proof: Option<ProofNode>) -> SearchOutcome {
        SearchOutcome {
            proof: (!self.exhausted).then_some(proof).flatten(),
            usage: self.usage,
            exhausted: self.exhausted,
        }
    }
}

fn cited_proof(
    proposition: &semantic_vocabulary::Proposition,
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
