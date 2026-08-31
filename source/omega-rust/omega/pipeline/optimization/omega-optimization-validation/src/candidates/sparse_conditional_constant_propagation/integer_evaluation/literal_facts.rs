//! Independent scalar-fact reconstruction used by integer and Boolean SCCP replay.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::{OptimizationUnitIdentity, ScalarConstantFactIdentity};
use omega_optimization_unit::{
    IntegerEvaluationWitness, PsiOptimizationFunction, PsiRewriteCandidate, ScalarConstantValue,
    ValueDefinitionSite, literal_scalar_constant_fact_identity,
};
use psi_core::{IntegerValue, ValueId};

use crate::OptimizationUnitValidationError;

use super::super::{scalar_value_definition, validator_scalar_constant_facts};

pub(super) fn binary_integer_operands(
    function: &PsiOptimizationFunction,
    candidate: &PsiRewriteCandidate,
    left: ValueId,
    right: ValueId,
) -> Result<(IntegerValue, IntegerValue), OptimizationUnitValidationError> {
    let Some((left_fact, right_fact)) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::binary_operands)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let left = literal_integer_fact(function, candidate.input(), left, left_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right = literal_integer_fact(function, candidate.input(), right, right_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    Ok((left, right))
}

pub(crate) fn unary_integer_operand(
    function: &PsiOptimizationFunction,
    candidate: &PsiRewriteCandidate,
    operand: ValueId,
) -> Result<IntegerValue, OptimizationUnitValidationError> {
    let Some(operand_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    literal_integer_fact(function, candidate.input(), operand, operand_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)
}

pub(crate) fn literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: OptimizationUnitIdentity,
    value: ValueId,
    identity: ScalarConstantFactIdentity,
) -> Option<IntegerValue> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Integer(value) => Some(value),
                    ScalarConstantValue::Boolean(_) => None,
                })
        })
}

pub(crate) fn direct_literal_integer_fact(
    function: &PsiOptimizationFunction,
    input: OptimizationUnitIdentity,
    value: ValueId,
    identity: ScalarConstantFactIdentity,
) -> Option<IntegerValue> {
    let definition = scalar_value_definition(function, value)?;
    let ValueDefinitionSite::Node { block, node } = definition.site else {
        return None;
    };
    let operation = &function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .nodes
        .get(usize::try_from(node).ok()?)?
        .operation;
    let O::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value: constant,
    } = operation
    else {
        return None;
    };
    if *result != value || *scalar_type != definition.scalar_type {
        return None;
    }
    let expected = literal_scalar_constant_fact_identity(
        input,
        function.machine,
        definition,
        ScalarConstantValue::Integer(*constant),
        *psi_operation,
    )?;
    (identity == expected).then_some(*constant)
}

pub(crate) fn literal_boolean_fact(
    function: &PsiOptimizationFunction,
    input: OptimizationUnitIdentity,
    value: ValueId,
    identity: ScalarConstantFactIdentity,
) -> Option<bool> {
    validator_scalar_constant_facts(input, function)
        .into_iter()
        .find_map(|(fact_value, constant, fact_identity)| {
            (fact_value == value && fact_identity == identity)
                .then_some(constant)
                .and_then(|constant| match constant {
                    ScalarConstantValue::Boolean(value) => Some(value),
                    ScalarConstantValue::Integer(_) => None,
                })
        })
}
