//! Selection support is a proof obligation, not a list of convenient cases.
//! A good candidate cannot hide an incompatible candidate under the same
//! choice. Shared condition DAGs retain correlation without actual products.

use super::bindings::Alias;
use super::transport::ConditionalPredicate;
use super::{BoundPredicate, CapturedBinding, CheckFacts, PredicateTransport};
use crate::flow::{SelectionCondition, SelectionConditions};

pub(super) fn implies(
    facts: &CheckFacts,
    conditions: &mut SelectionConditions,
    premise: SelectionCondition,
    conclusion: SelectionCondition,
) -> bool {
    let missing = conditions.negate(&facts.flow.control, conclusion);
    let counterexample = conditions.all(premise, missing);
    !conditions.possible(&facts.flow.control, counterexample)
}

pub(super) fn dependency(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    captures: &[CapturedBinding<'_>],
    selections: &[SelectionCondition],
    conditions: &mut SelectionConditions,
    source: &Alias<'_>,
    position: usize,
) -> SelectionCondition {
    let mut good = conditions.never();
    let mut bad = conditions.never();
    for (candidate, &selection) in captures.iter().zip(selections) {
        if candidate.position != position {
            continue;
        }
        let agrees = candidate
            .aliases
            .iter()
            .any(|alias| aliases_agree(program, facts, source, alias));
        if agrees {
            good = conditions.any(good, selection);
        } else {
            bad = conditions.any(bad, selection);
        }
    }
    let not_bad = conditions.negate(&facts.flow.control, bad);
    conditions.all(good, not_bad)
}

fn aliases_agree(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    left: &Alias<'_>,
    right: &Alias<'_>,
) -> bool {
    left.place == right.place
        && crate::flow::operand_observations_agree(
            program,
            &facts.flow,
            left.epoch,
            right.epoch,
            &left.place,
        )
}

pub(super) fn join_identities(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    required: &mut BoundPredicate,
    captures: &[CapturedBinding<'_>],
    selections: &[SelectionCondition],
    conditions: &mut SelectionConditions,
) {
    for right in 0..required.bindings.len() {
        for left in 0..right {
            let source = required.bindings[right].clone();
            let destination = required.bindings[left].clone();
            if source.place == destination.place
                || source.operand.primitive_type() != destination.operand.primitive_type()
            {
                continue;
            }
            let mut good = conditions.never();
            let mut bad = conditions.never();
            for (first, &first_condition) in captures
                .iter()
                .zip(selections)
                .filter(|(capture, _)| capture.position == left)
            {
                for (second, &second_condition) in captures
                    .iter()
                    .zip(selections)
                    .filter(|(capture, _)| capture.position == right)
                {
                    let selection = conditions.all(first_condition, second_condition);
                    if !conditions.possible(&facts.flow.control, selection) {
                        continue;
                    }
                    let agrees = first.aliases.iter().any(|one| {
                        second
                            .aliases
                            .iter()
                            .any(|two| aliases_agree(program, facts, one, two))
                    });
                    if agrees {
                        good = conditions.any(good, selection);
                    } else {
                        bad = conditions.any(bad, selection);
                    }
                }
            }
            let not_bad = conditions.negate(&facts.flow.control, bad);
            let identity = conditions.all(good, not_bad);
            if conditions.covers(&facts.flow.control, identity) {
                // Normalize the whole exact observation class. This is value
                // substitution identity, including IEEE values, not a new
                // assertion that Float language equality is reflexive.
                for binding in &mut required.bindings {
                    if binding.place == source.place
                        && binding.operand.primitive_type() == source.operand.primitive_type()
                    {
                        binding.place = destination.place.clone();
                    }
                }
            }
        }
    }
}

pub(super) fn qualify(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    required: &BoundPredicate,
    premises: &[BoundPredicate],
    evidence: Vec<ConditionalPredicate>,
    conditions: &mut SelectionConditions,
) -> PredicateTransport {
    let mut result = PredicateTransport::default();
    for proposition in &evidence {
        if conditions.covers(&facts.flow.control, proposition.condition) {
            result.predicates.push(proposition.predicate.clone());
        }
    }
    let mut proven = conditions.never();
    for support in &evidence {
        let mut hypotheses = premises.to_vec();
        for proposition in &evidence {
            if implies(facts, conditions, support.condition, proposition.condition) {
                hypotheses.push(proposition.predicate.clone());
            }
        }
        // Each implication uses only premises established throughout this
        // support. The requested proposition is never among its assumptions.
        if super::super::prover::call_guarantees::proves_captured_predicates(
            program,
            &facts.semantic,
            &[],
            &hypotheses,
            required,
            |_| None,
        ) {
            proven = conditions.any(proven, support.condition);
        }
    }
    result.proves_required = conditions.covers(&facts.flow.control, proven);
    result
}
