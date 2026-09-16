//! The integer bound rules: affine and cast bounds from a root bound, the
//! exact-add definition bound, and correlated forbidden roots; each records
//! the semantic axioms its witness cites.

use super::integer_math_normalization::lower_integer_math_relation;
use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
use crate::{
    IntegerAffineBoundConversionError, IntegerAffineWitness, IntegerCastBoundConversionError,
    check_integer_affine_bound_conversion, check_integer_affine_witness,
    check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
    check_integer_correlated_forbidden_root_conversion,
    check_integer_correlated_forbidden_root_witness, integer_affine_truth_bounds,
    integer_cast_truth_bounds, map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarTerm};
use terminal_psi::{ProofNode, ProofRule};

pub(super) fn check_integer_affine_bound(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::IntegerAffineBound {
        root_bound,
        witness,
    } = &proof.rule
    else {
        unreachable!("dispatched check_integer_affine_bound")
    };
    let RuleScope {
        context,
        semantic_axioms,
        ..
    } = *scope;
    acceptance
        .rules
        .insert(AcceptedProofRule::IntegerAffineBound);
    let form = check_integer_affine_witness(context, semantic_axioms, witness)
        .map_err(ProofError::IntegerAffineWitness)?;
    let normalized_conclusion =
        lower_integer_math_relation(&proof.conclusion).unwrap_or_else(|| proof.conclusion.clone());
    if root_bound.conclusion == Proposition::Truth {
        let bounds =
            integer_affine_truth_bounds(&form).map_err(ProofError::IntegerAffineBoundConversion)?;
        if !bounds.contains(&normalized_conclusion) {
            return Err(ProofError::IntegerAffineBoundConversion(
                IntegerAffineBoundConversionError::ConclusionMismatch,
            ));
        }
    } else {
        check_integer_affine_bound_conversion(
            &form,
            &root_bound.conclusion,
            &normalized_conclusion,
        )
        .map_err(ProofError::IntegerAffineBoundConversion)?;
    }
    for (&definition_index, &literal_index) in witness
        .definition_axioms
        .iter()
        .zip(&witness.literal_axioms)
    {
        if let Some(index) = literal_index {
            let proposition = semantic_axioms
                .get(index)
                .ok_or(ProofError::UnknownSemanticAxiom(index))?;
            acceptance.record_semantic_axiom(index, proposition);
        }
        let proposition = semantic_axioms
            .get(definition_index)
            .ok_or(ProofError::UnknownSemanticAxiom(definition_index))?;
        acceptance.record_semantic_axiom(definition_index, proposition);
    }
    Ok(())
}

pub(super) fn check_integer_exact_add_definition_bound(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::IntegerExactAddDefinitionBound {
        left_bound,
        right_bound,
        definition_axiom,
    } = &proof.rule
    else {
        unreachable!("dispatched check_integer_exact_add_definition_bound")
    };
    let RuleScope {
        context,
        semantic_axioms,
        ..
    } = *scope;
    acceptance
        .rules
        .insert(AcceptedProofRule::IntegerExactAddDefinitionBound);
    let definition = semantic_axioms
        .get(*definition_axiom)
        .ok_or(ProofError::UnknownSemanticAxiom(*definition_axiom))?;
    context
        .validate(definition)
        .map_err(ProofError::MalformedProposition)?;
    let Proposition::Equal(first, second) = definition else {
        return Err(ProofError::RulePremiseMismatch(
            "integer exact-add definition",
        ));
    };
    let (output, expression) = match (first, second) {
        (ScalarTerm::Value { .. }, ScalarTerm::ExactIntegerAdd { .. }) => (first, second),
        (ScalarTerm::ExactIntegerAdd { .. }, ScalarTerm::Value { .. }) => (second, first),
        _ => {
            return Err(ProofError::RulePremiseMismatch(
                "integer exact-add definition",
            ));
        }
    };
    let ScalarTerm::ExactIntegerAdd {
        scalar_type, left, ..
    } = expression
    else {
        unreachable!("matched exact-add definition")
    };
    if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || output.scalar_type() != semantic_vocabulary::ScalarType::Integer(*scalar_type)
    {
        return Err(ProofError::RulePremiseMismatch(
            "integer exact-add definition type",
        ));
    }
    let witness = IntegerAffineWitness {
        root: left.as_ref().clone(),
        target: expression.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness)
        .map_err(ProofError::IntegerAffineWitness)?;
    let evidence = Proposition::Conjunction(vec![
        left_bound.conclusion.clone(),
        right_bound.conclusion.clone(),
    ]);
    let mapped = map_integer_affine_bound(&form, &evidence)
        .map_err(ProofError::IntegerAffineBoundConversion)?;
    let Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right) = mapped else {
        return Err(ProofError::RulePremiseMismatch(
            "integer exact-add mapped bound",
        ));
    };
    let (literal, lower) = match (&mapped_left, &mapped_right) {
        (IntegerMathTerm::IntegerLiteral(literal), IntegerMathTerm::Add(_, _)) => (literal, true),
        (IntegerMathTerm::Add(_, _), IntegerMathTerm::IntegerLiteral(literal)) => (literal, false),
        _ => {
            return Err(ProofError::RulePremiseMismatch(
                "integer exact-add mapped bound",
            ));
        }
    };
    let value = literal
        .as_integer_value(*scalar_type)
        .ok_or(ProofError::RulePremiseMismatch(
            "integer exact-add mapped literal",
        ))?;
    let literal = ScalarTerm::integer(*scalar_type, value)
        .map_err(|_| ProofError::RulePremiseMismatch("integer exact-add mapped literal"))?;
    let expected = if lower {
        Proposition::LessOrEqual(literal, output.clone())
    } else {
        Proposition::LessOrEqual(output.clone(), literal)
    };
    if proof.conclusion != expected {
        return Err(ProofError::IntegerAffineBoundConversion(
            IntegerAffineBoundConversionError::ConclusionMismatch,
        ));
    }
    acceptance.record_semantic_axiom(*definition_axiom, definition);
    Ok(())
}

pub(super) fn check_integer_cast_bound(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::IntegerCastBound {
        root_bound,
        witness,
    } = &proof.rule
    else {
        unreachable!("dispatched check_integer_cast_bound")
    };
    let RuleScope {
        context,
        semantic_axioms,
        ..
    } = *scope;
    acceptance.rules.insert(AcceptedProofRule::IntegerCastBound);
    let chain = check_integer_cast_chain_witness(context, semantic_axioms, witness)
        .map_err(ProofError::IntegerCastChainWitness)?;
    let normalized_conclusion =
        lower_integer_math_relation(&proof.conclusion).unwrap_or_else(|| proof.conclusion.clone());
    if root_bound.conclusion == Proposition::Truth {
        let bounds =
            integer_cast_truth_bounds(&chain).map_err(ProofError::IntegerCastBoundConversion)?;
        if !bounds.contains(&normalized_conclusion) {
            return Err(ProofError::IntegerCastBoundConversion(
                IntegerCastBoundConversionError::ConclusionLiteralMismatch,
            ));
        }
    } else {
        check_integer_cast_bound_conversion(&chain, &root_bound.conclusion, &normalized_conclusion)
            .map_err(ProofError::IntegerCastBoundConversion)?;
    }
    for &index in &witness.definition_axioms {
        let proposition = semantic_axioms
            .get(index)
            .ok_or(ProofError::UnknownSemanticAxiom(index))?;
        acceptance.record_semantic_axiom(index, proposition);
    }
    Ok(())
}

pub(super) fn check_integer_correlated_forbidden_roots(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let ProofRule::IntegerCorrelatedForbiddenRoots { witness } = &proof.rule else {
        unreachable!("dispatched check_integer_correlated_forbidden_roots")
    };
    let RuleScope {
        context,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        ..
    } = *scope;
    acceptance
        .rules
        .insert(AcceptedProofRule::IntegerCorrelatedForbiddenRoots);
    if witness.definition_axiom_count != semantic_axioms.len() {
        return Err(ProofError::IntegerCorrelatedForbiddenRootDefinitionBoundary);
    }
    let mut ledger = Vec::with_capacity(semantic_axioms.len() + assumptions.len());
    ledger.extend_from_slice(semantic_axioms);
    ledger.extend_from_slice(assumptions);
    let checked = check_integer_correlated_forbidden_root_witness(
        context,
        &ledger,
        machine_parameter_values,
        witness,
    )
    .map_err(ProofError::IntegerCorrelatedForbiddenRootWitness)?;
    check_integer_correlated_forbidden_root_conversion(&checked, &proof.conclusion)
        .map_err(ProofError::IntegerCorrelatedForbiddenRootConversion)?;

    for branch in [&witness.dividend, &witness.divisor] {
        for step in &branch.steps {
            if let Some(index) = step.literal_axiom {
                let proposition = semantic_axioms
                    .get(index)
                    .ok_or(ProofError::UnknownSemanticAxiom(index))?;
                acceptance.record_semantic_axiom(index, proposition);
            }
            let index = step.definition_axiom;
            let proposition = semantic_axioms
                .get(index)
                .ok_or(ProofError::UnknownSemanticAxiom(index))?;
            acceptance.record_semantic_axiom(index, proposition);
        }
    }
    let (lower_bound, upper_bound) = checked.bound_axioms();
    for ledger_index in [lower_bound, upper_bound] {
        let index = ledger_index
            .checked_sub(semantic_axioms.len())
            .ok_or(ProofError::IntegerCorrelatedForbiddenRootRequirementBoundary)?;
        let proposition = assumptions
            .get(index)
            .ok_or(ProofError::UnknownAssumption(index))?;
        acceptance.record_assumption(index, proposition);
    }
    Ok(())
}
