//! The integer bound rules: affine and cast bounds from a root bound, the
//! exact-add and exact-subtract definition bounds, and correlated forbidden
//! roots; each records the semantic axioms its witness cites.
//!
//! Each rule's premise/conclusion relation is a `pub(crate)` function that
//! returns the roster indices its witness cites — in the same order the
//! checker records them — so the mathematical-core denotation re-decides
//! the exact check and binds the same citations as premises of the
//! elaborated rule instance.

use std::collections::BTreeSet;

use super::integer_math_normalization::lower_integer_math_relation;
use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
use crate::{
    CheckedIntegerCastChain, CheckedIntegerCorrelatedForbiddenRoots,
    IntegerAffineBoundConversionError, IntegerAffineWitness, IntegerCastBoundConversionError,
    IntegerCorrelatedForbiddenRootWitness, check_integer_affine_bound_conversion,
    check_integer_affine_witness, check_integer_cast_bound_conversion,
    check_integer_cast_chain_witness, check_integer_correlated_forbidden_root_conversion,
    check_integer_correlated_forbidden_root_witness, integer_affine_truth_bounds,
    integer_cast_truth_bounds, map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext, ScalarTerm, ValueId};
use terminal_psi::{ProofNode, ProofRule};

/// The `IntegerAffineBound` premise/conclusion relation: the checked
/// affine form mapped over the root bound must produce the normalized
/// conclusion. Returns the semantic-axiom indices the witness cites, in
/// recording order — literal axiom before its definition axiom, per pair.
pub(crate) fn affine_bound_relation(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root_bound: &Proposition,
    witness: &IntegerAffineWitness,
    conclusion: &Proposition,
) -> Result<Vec<usize>, ProofError> {
    let form = check_integer_affine_witness(context, semantic_axioms, witness)
        .map_err(ProofError::IntegerAffineWitness)?;
    let normalized_conclusion =
        lower_integer_math_relation(conclusion).unwrap_or_else(|| conclusion.clone());
    if *root_bound == Proposition::Truth {
        let bounds =
            integer_affine_truth_bounds(&form).map_err(ProofError::IntegerAffineBoundConversion)?;
        if !bounds.contains(&normalized_conclusion) {
            return Err(ProofError::IntegerAffineBoundConversion(
                IntegerAffineBoundConversionError::ConclusionMismatch,
            ));
        }
    } else {
        check_integer_affine_bound_conversion(&form, root_bound, &normalized_conclusion)
            .map_err(ProofError::IntegerAffineBoundConversion)?;
    }
    let mut cited = Vec::new();
    for (&definition_index, &literal_index) in witness
        .definition_axioms
        .iter()
        .zip(&witness.literal_axioms)
    {
        if let Some(index) = literal_index {
            if index >= semantic_axioms.len() {
                return Err(ProofError::UnknownSemanticAxiom(index));
            }
            cited.push(index);
        }
        if definition_index >= semantic_axioms.len() {
            return Err(ProofError::UnknownSemanticAxiom(definition_index));
        }
        cited.push(definition_index);
    }
    Ok(cited)
}

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
    let cited = affine_bound_relation(
        context,
        semantic_axioms,
        &root_bound.conclusion,
        witness,
        &proof.conclusion,
    )?;
    for index in cited {
        let proposition = semantic_axioms
            .get(index)
            .expect("the relation bounds-checked every cited index");
        acceptance.record_semantic_axiom(index, proposition);
    }
    Ok(())
}

/// The exact operation a definition-bound rule maps its two operand bounds
/// through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactDefinitionOperation {
    Add,
    Subtract,
}

impl ExactDefinitionOperation {
    fn definition_label(self) -> &'static str {
        match self {
            Self::Add => "integer exact-add definition",
            Self::Subtract => "integer exact-subtract definition",
        }
    }

    fn type_label(self) -> &'static str {
        match self {
            Self::Add => "integer exact-add definition type",
            Self::Subtract => "integer exact-subtract definition type",
        }
    }

    fn mapped_label(self) -> &'static str {
        match self {
            Self::Add => "integer exact-add mapped bound",
            Self::Subtract => "integer exact-subtract mapped bound",
        }
    }

    fn literal_label(self) -> &'static str {
        match self {
            Self::Add => "integer exact-add mapped literal",
            Self::Subtract => "integer exact-subtract mapped literal",
        }
    }

    /// The fixed scalar type and left operand of `expression` when it is
    /// this operation.
    fn operands(
        self,
        expression: &ScalarTerm,
    ) -> Option<(semantic_vocabulary::IntegerType, &ScalarTerm)> {
        match (self, expression) {
            (
                Self::Add,
                ScalarTerm::ExactIntegerAdd {
                    scalar_type, left, ..
                },
            )
            | (
                Self::Subtract,
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type, left, ..
                },
            ) => Some((*scalar_type, left.as_ref())),
            _ => None,
        }
    }

    fn is_mapped_term(self, term: &IntegerMathTerm) -> bool {
        matches!(
            (self, term),
            (Self::Add, IntegerMathTerm::Add(_, _))
                | (Self::Subtract, IntegerMathTerm::Subtract(_, _))
        )
    }
}

/// The exact-definition bound premise/conclusion relation shared by
/// `IntegerExactAddDefinitionBound` and `IntegerExactSubtractDefinitionBound`:
/// the cited definition must be a value/exact-operation equality over a
/// fixed carrier, and the two proved bounds mapped through the operation's
/// affine form must land the conclusion on the definition's output.
pub(crate) fn exact_definition_bound_relation(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    left_bound: &Proposition,
    right_bound: &Proposition,
    definition_axiom: usize,
    conclusion: &Proposition,
    operation: ExactDefinitionOperation,
) -> Result<(), ProofError> {
    let definition = semantic_axioms
        .get(definition_axiom)
        .ok_or(ProofError::UnknownSemanticAxiom(definition_axiom))?;
    context
        .validate(definition)
        .map_err(ProofError::MalformedProposition)?;
    let Proposition::Equal(first, second) = definition else {
        return Err(ProofError::RulePremiseMismatch(
            operation.definition_label(),
        ));
    };
    let (output, expression) = match (first, second) {
        (ScalarTerm::Value { .. }, expression) if operation.operands(expression).is_some() => {
            (first, second)
        }
        (expression, ScalarTerm::Value { .. }) if operation.operands(expression).is_some() => {
            (second, first)
        }
        _ => {
            return Err(ProofError::RulePremiseMismatch(
                operation.definition_label(),
            ));
        }
    };
    let (scalar_type, left) = operation
        .operands(expression)
        .expect("matched the exact-operation definition");
    if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || output.scalar_type() != semantic_vocabulary::ScalarType::Integer(scalar_type)
    {
        return Err(ProofError::RulePremiseMismatch(operation.type_label()));
    }
    let witness = IntegerAffineWitness {
        root: left.clone(),
        target: expression.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness)
        .map_err(ProofError::IntegerAffineWitness)?;
    let evidence = Proposition::Conjunction(vec![left_bound.clone(), right_bound.clone()]);
    let mapped = map_integer_affine_bound(&form, &evidence)
        .map_err(ProofError::IntegerAffineBoundConversion)?;
    let Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right) = mapped else {
        return Err(ProofError::RulePremiseMismatch(operation.mapped_label()));
    };
    let (literal, lower) = match (&mapped_left, &mapped_right) {
        (IntegerMathTerm::IntegerLiteral(literal), term) if operation.is_mapped_term(term) => {
            (literal, true)
        }
        (term, IntegerMathTerm::IntegerLiteral(literal)) if operation.is_mapped_term(term) => {
            (literal, false)
        }
        _ => {
            return Err(ProofError::RulePremiseMismatch(operation.mapped_label()));
        }
    };
    let value = literal
        .as_integer_value(scalar_type)
        .ok_or(ProofError::RulePremiseMismatch(operation.literal_label()))?;
    let literal = ScalarTerm::integer(scalar_type, value)
        .map_err(|_| ProofError::RulePremiseMismatch(operation.literal_label()))?;
    let expected = if lower {
        Proposition::LessOrEqual(literal, output.clone())
    } else {
        Proposition::LessOrEqual(output.clone(), literal)
    };
    if *conclusion != expected {
        return Err(ProofError::IntegerAffineBoundConversion(
            IntegerAffineBoundConversionError::ConclusionMismatch,
        ));
    }
    Ok(())
}

/// Check an exact-definition bound node: the shared relation for its
/// operation, recording the rule and its cited definition.
pub(super) fn check_integer_exact_definition_bound(
    scope: &RuleScope<'_>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let (left_bound, right_bound, definition_axiom, operation, rule) = match &proof.rule {
        ProofRule::IntegerExactAddDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        } => (
            left_bound,
            right_bound,
            definition_axiom,
            ExactDefinitionOperation::Add,
            AcceptedProofRule::IntegerExactAddDefinitionBound,
        ),
        ProofRule::IntegerExactSubtractDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        } => (
            left_bound,
            right_bound,
            definition_axiom,
            ExactDefinitionOperation::Subtract,
            AcceptedProofRule::IntegerExactSubtractDefinitionBound,
        ),
        _ => unreachable!("dispatched check_integer_exact_definition_bound"),
    };
    let RuleScope {
        context,
        semantic_axioms,
        ..
    } = *scope;
    acceptance.rules.insert(rule);
    exact_definition_bound_relation(
        context,
        semantic_axioms,
        &left_bound.conclusion,
        &right_bound.conclusion,
        *definition_axiom,
        &proof.conclusion,
        operation,
    )?;
    let definition = semantic_axioms
        .get(*definition_axiom)
        .expect("the relation bounds-checked the cited definition");
    acceptance.record_semantic_axiom(*definition_axiom, definition);
    Ok(())
}

/// The `IntegerCastBound` premise/conclusion relation: the checked cast
/// chain mapped over the root bound must produce the normalized
/// conclusion. The cited semantic axioms are `witness.definition_axioms`.
pub(crate) fn cast_bound_relation(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root_bound: &Proposition,
    witness: &crate::IntegerCastChainWitness,
    conclusion: &Proposition,
) -> Result<CheckedIntegerCastChain, ProofError> {
    let chain = check_integer_cast_chain_witness(context, semantic_axioms, witness)
        .map_err(ProofError::IntegerCastChainWitness)?;
    let normalized_conclusion =
        lower_integer_math_relation(conclusion).unwrap_or_else(|| conclusion.clone());
    if *root_bound == Proposition::Truth {
        let bounds =
            integer_cast_truth_bounds(&chain).map_err(ProofError::IntegerCastBoundConversion)?;
        if !bounds.contains(&normalized_conclusion) {
            return Err(ProofError::IntegerCastBoundConversion(
                IntegerCastBoundConversionError::ConclusionLiteralMismatch,
            ));
        }
    } else {
        check_integer_cast_bound_conversion(&chain, root_bound, &normalized_conclusion)
            .map_err(ProofError::IntegerCastBoundConversion)?;
    }
    for &index in &witness.definition_axioms {
        if index >= semantic_axioms.len() {
            return Err(ProofError::UnknownSemanticAxiom(index));
        }
    }
    Ok(chain)
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
    cast_bound_relation(
        context,
        semantic_axioms,
        &root_bound.conclusion,
        witness,
        &proof.conclusion,
    )?;
    for &index in &witness.definition_axioms {
        let proposition = semantic_axioms
            .get(index)
            .expect("the relation bounds-checked every cited index");
        acceptance.record_semantic_axiom(index, proposition);
    }
    Ok(())
}

/// The roster citations a checked `IntegerCorrelatedForbiddenRoots`
/// instance commits to: the semantic axioms each branch step cites
/// (literal axiom before definition axiom, dividend branch then divisor),
/// then the ambient assumption indices the bound axioms name.
#[derive(Debug, Default)]
pub(crate) struct CorrelatedCitations {
    pub semantic_axioms: Vec<usize>,
    pub assumptions: Vec<usize>,
}

/// The `IntegerCorrelatedForbiddenRoots` premise/conclusion relation: the
/// witness's two correlated affine branches replay over the axiom and
/// assumption ledger under the machine-parameter values, and the checked
/// interval must produce the conclusion. Returns the citation indices
/// and the checked witness the denotation layer replays.
pub(crate) fn correlated_forbidden_roots_relation(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    witness: &IntegerCorrelatedForbiddenRootWitness,
    conclusion: &Proposition,
) -> Result<(CorrelatedCitations, CheckedIntegerCorrelatedForbiddenRoots), ProofError> {
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
    check_integer_correlated_forbidden_root_conversion(&checked, conclusion)
        .map_err(ProofError::IntegerCorrelatedForbiddenRootConversion)?;

    let mut citations = CorrelatedCitations::default();
    for branch in [&witness.dividend, &witness.divisor] {
        for step in &branch.steps {
            if let Some(index) = step.literal_axiom {
                if index >= semantic_axioms.len() {
                    return Err(ProofError::UnknownSemanticAxiom(index));
                }
                citations.semantic_axioms.push(index);
            }
            let index = step.definition_axiom;
            if index >= semantic_axioms.len() {
                return Err(ProofError::UnknownSemanticAxiom(index));
            }
            citations.semantic_axioms.push(index);
        }
    }
    let (lower_bound, upper_bound) = checked.bound_axioms();
    for ledger_index in [lower_bound, upper_bound] {
        let index = ledger_index
            .checked_sub(semantic_axioms.len())
            .ok_or(ProofError::IntegerCorrelatedForbiddenRootRequirementBoundary)?;
        if index >= assumptions.len() {
            return Err(ProofError::UnknownAssumption(index));
        }
        citations.assumptions.push(index);
    }
    Ok((citations, checked))
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
    let (citations, _) = correlated_forbidden_roots_relation(
        context,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        witness,
        &proof.conclusion,
    )?;
    for index in citations.semantic_axioms {
        let proposition = semantic_axioms
            .get(index)
            .expect("the relation bounds-checked every cited index");
        acceptance.record_semantic_axiom(index, proposition);
    }
    for index in citations.assumptions {
        let proposition = assumptions
            .get(index)
            .expect("the relation bounds-checked every cited index");
        acceptance.record_assumption(index, proposition);
    }
    Ok(())
}
