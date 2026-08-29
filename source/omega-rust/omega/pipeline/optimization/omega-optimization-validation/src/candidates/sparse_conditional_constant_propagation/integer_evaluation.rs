//! Independent exact integer evaluation and literal reconstruction.

use super::*;

pub(crate) fn evaluate_integer_operation(
    function: &PsiOptimizationFunction,
    node: &omega_optimization_unit::OptimizationNode,
    candidate: &PsiRewriteCandidate,
) -> Result<
    (
        psi_core::OperationId,
        ValueId,
        psi_core::IntegerType,
        psi_core::IntegerValue,
        OptimizationSafetyClass,
    ),
    OptimizationUnitValidationError,
> {
    use omega_abstract_operations::AbstractOperation as O;
    if let O::IntegerExactCast {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
        ..
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .exact_cast_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ProofCertified,
        ));
    }
    if let O::IntegerWiden {
        psi_operation,
        result,
        source_type,
        target_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = source_type
            .widen_value_to(target_type, operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            target_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    if let O::IntegerBitwiseNot {
        psi_operation,
        result,
        scalar_type,
        operand,
    } = node.operation
    {
        let operand_value = unary_integer_operand(function, candidate, operand)?;
        let evaluated = scalar_type
            .bitwise_not(operand_value)
            .ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
        return Ok((
            psi_operation,
            result,
            scalar_type,
            evaluated,
            OptimizationSafetyClass::ExactOperationSemantics,
        ));
    }
    enum IntegerOperation {
        ExactAdd,
        ExactSubtract,
        ExactMultiply,
        WrappingAdd,
        WrappingSubtract,
        WrappingMultiply,
        SaturatingAdd,
        SaturatingSubtract,
        SaturatingMultiply,
        ExactDivide,
        ExactRemainder,
        WrappingDivide,
        WrappingRemainder,
        SaturatingDivide,
        SaturatingRemainder,
        ExactShiftLeft(psi_core::IntegerType),
        ExactShiftRight(psi_core::IntegerType),
        WrappingShiftLeft(psi_core::IntegerType),
        WrappingShiftRight(psi_core::IntegerType),
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
    }
    let (kind, source, result, scalar_type, left, right) = match &node.operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::WrappingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingAdd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingSubtract,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::SaturatingMultiply,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::ExactRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::WrappingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingDivide,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => (
            IntegerOperation::SaturatingRemainder,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (
            IntegerOperation::ExactShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftLeft(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IntegerOperation::WrappingShiftRight(*count_type),
            *psi_operation,
            *result,
            *value_type,
            *value,
            *count,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseAnd,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseOr,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IntegerOperation::BitwiseXor,
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let Some((left_fact, right_fact)) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::binary_operands)
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let left_value = literal_integer_fact(function, candidate.input(), left, left_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let right_value = literal_integer_fact(function, candidate.input(), right, right_fact)
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let (evaluated, safety_class) = match kind {
        IntegerOperation::ExactAdd => (
            scalar_type.exact_add(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactSubtract => (
            scalar_type.exact_sub(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactMultiply => (
            scalar_type.exact_mul(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingAdd => (
            scalar_type.wrapping_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingSubtract => (
            scalar_type.wrapping_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingMultiply => (
            scalar_type.wrapping_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingAdd => (
            scalar_type.saturating_add(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingSubtract => (
            scalar_type.saturating_sub(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::SaturatingMultiply => (
            scalar_type.saturating_mul(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::ExactDivide => (
            scalar_type.exact_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactRemainder => (
            scalar_type.exact_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingDivide => (
            scalar_type.wrapping_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingRemainder => (
            scalar_type.wrapping_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingDivide => (
            scalar_type.saturating_div(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::SaturatingRemainder => (
            scalar_type.saturating_rem(left_value, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftLeft(count_type) => (
            scalar_type.exact_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::ExactShiftRight(count_type) => (
            scalar_type.exact_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ProofCertified,
        ),
        IntegerOperation::WrappingShiftLeft(count_type) => (
            scalar_type.wrapping_shift_left(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::WrappingShiftRight(count_type) => (
            scalar_type.wrapping_shift_right(left_value, count_type, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseAnd => (
            scalar_type.bitwise_and(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseOr => (
            scalar_type.bitwise_or(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
        IntegerOperation::BitwiseXor => (
            scalar_type.bitwise_xor(left_value, right_value),
            OptimizationSafetyClass::ExactOperationSemantics,
        ),
    };
    let evaluated =
        evaluated.ok_or(OptimizationUnitValidationError::CandidateEvaluationMismatch)?;
    Ok((source, result, scalar_type, evaluated, safety_class))
}

pub(crate) fn unary_integer_operand(
    function: &PsiOptimizationFunction,
    candidate: &PsiRewriteCandidate,
    operand: ValueId,
) -> Result<psi_core::IntegerValue, OptimizationUnitValidationError> {
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
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
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
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
) -> Option<psi_core::IntegerValue> {
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
    input: omega_optimization_core::OptimizationUnitIdentity,
    value: ValueId,
    identity: omega_optimization_core::ScalarConstantFactIdentity,
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
