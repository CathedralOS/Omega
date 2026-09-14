//! Operation-local polarity facts from the validated scalar denotation.
//!
//! No goal, caller premise, or neighboring operation participates in this
//! projection. The original result equation remains the first emitted fact.
//!
//! Each emitted polarity implication is discharged, not trusted: a canonical
//! derivation of fixed shape is produced by this total certifying procedure
//! and re-decided by the proof-admission certificate checker before the
//! implication may join the premise roster. Implication introduction binds the
//! normalized denotation premise as assumption 0; predicate-denotation
//! conversion rewrites that hypothesis to the `denotation == polarity`
//! proposition; equality transitivity composes it with the result-equation
//! axiom (semantic axiom 0); implication introduction then discharges the
//! hypothesis. A malformed roster, a wrong pairing, or any checker rejection
//! fails generation closed rather than emitting an undischarged premise.

use std::collections::{BTreeMap, BTreeSet};

use proof_admission::{
    PredicateDenotationError, ProofNode, ProofRule, check_certificate, check_predicate_denotations,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};
use terminal_semantics::{GoalFreeScalarLeafSemantics, ScalarLeafFactShape};

use crate::ModuleError;

/// The canonical derivation discharging one polarity implication. Every node
/// conclusion is bound exactly; the checker re-decides each step, so a caller
/// may verify the emitted fact rather than trust its construction.
fn polarity_certificate(
    equation: &Proposition,
    denotation_polarity: &Proposition,
    normalized_premise: &Proposition,
    conclusion: &Proposition,
    goal: &Proposition,
) -> ProofNode {
    // Under implication introduction the pushed hypothesis is assumption 0:
    // this derivation is always checked with an empty ambient roster.
    let hypothesis = ProofNode {
        conclusion: normalized_premise.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    let converted = ProofNode {
        conclusion: denotation_polarity.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(hypothesis),
        },
    };
    // The leaf's own result equation is the only semantic axiom this
    // derivation cites.
    let equation_axiom = ProofNode {
        conclusion: equation.clone(),
        rule: ProofRule::SemanticAxiom { index: 0 },
    };
    let body = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(equation_axiom),
            middle_equals_right: Box::new(converted),
        },
    };
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(body),
        },
    }
}

fn implication(
    context: &PropositionContext,
    equation: &Proposition,
    result: &ScalarTerm,
    denotation: &ScalarTerm,
    positive: bool,
) -> Result<Proposition, ModuleError> {
    let denotation_polarity = Proposition::Equal(denotation.clone(), ScalarTerm::boolean(positive));
    let checked = check_predicate_denotations(context, &denotation_polarity, &[], &[])
        .map_err(|error| ModuleError::OperationPredicateDenotation(Box::new(error)))?;
    let conclusion = Proposition::Equal(result.clone(), ScalarTerm::boolean(positive));
    let implication = Proposition::Implication {
        premise: Box::new(checked.goal().clone()),
        conclusion: Box::new(conclusion.clone()),
    };
    let certificate = polarity_certificate(
        equation,
        &denotation_polarity,
        checked.goal(),
        &conclusion,
        &implication,
    );
    check_certificate(
        context,
        &implication,
        &[],
        std::slice::from_ref(equation),
        &certificate,
    )
    .map_err(|error| {
        ModuleError::OperationPredicateDenotation(Box::new(PredicateDenotationError::Proof(error)))
    })?;
    Ok(implication)
}

pub(super) fn implications(
    semantics: &GoalFreeScalarLeafSemantics,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Vec<Proposition>, ModuleError> {
    match semantics.fact_shape() {
        ScalarLeafFactShape::ResultEquation => return Ok(Vec::new()),
        ScalarLeafFactShape::BooleanResultEquationAndPolarityImplications => {}
    }
    let equation = semantics.result_equation();
    let Proposition::Equal(result, denotation) = equation else {
        unreachable!("validated scalar leaf observation retains its result equation")
    };
    // A scalar leaf has only its result and direct operands. Do not rebuild a
    // whole-machine proposition context separately for every operation.
    let mut values = BTreeSet::new();
    equation.visit_value_ids(|value| {
        values.insert(value);
    });
    let context = PropositionContext::from_value_types(values.iter().filter_map(|value| {
        value_types
            .get(value)
            .map(|scalar_type| (*value, *scalar_type))
    }))
    .map_err(ModuleError::MalformedProposition)?;
    context
        .validate(equation)
        .map_err(ModuleError::MalformedProposition)?;

    [true, false]
        .into_iter()
        .map(|positive| implication(&context, equation, result, denotation, positive))
        .collect()
}

#[cfg(test)]
mod tests;
