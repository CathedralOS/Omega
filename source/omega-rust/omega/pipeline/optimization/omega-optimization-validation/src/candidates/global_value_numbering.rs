//! Local, dominating, and phi-translated scalar CSE validation.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentTotalScalarExpressionKey {
    BooleanConstant(bool),
    IntegerConstant(ScalarType, psi_core::IntegerValue),
    BooleanNot(ValueId),
    BooleanEqual(ValueId, ValueId),
    IntegerEqual(IntegerType, ValueId, ValueId),
    IntegerLessThan(IntegerType, ValueId, ValueId),
    IntegerLessOrEqual(IntegerType, ValueId, ValueId),
    IntegerBitwiseNot(IntegerType, ValueId),
    IntegerWiden(IntegerType, IntegerType, ValueId),
    IntegerBitwiseAnd(IntegerType, ValueId, ValueId),
    IntegerBitwiseOr(IntegerType, ValueId, ValueId),
    IntegerBitwiseXor(IntegerType, ValueId, ValueId),
    WrappingShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    WrappingShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    WrappingAdd(IntegerType, ValueId, ValueId),
    WrappingSubtract(IntegerType, ValueId, ValueId),
    WrappingMultiply(IntegerType, ValueId, ValueId),
    SaturatingAdd(IntegerType, ValueId, ValueId),
    SaturatingSubtract(IntegerType, ValueId, ValueId),
    SaturatingMultiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentProofScalarExpressionKey {
    ExactCast(IntegerType, IntegerType, ValueId),
    ExactShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ExactShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    ExactAdd(IntegerType, ValueId, ValueId),
    ExactSubtract(IntegerType, ValueId, ValueId),
    ExactMultiply(IntegerType, ValueId, ValueId),
    ExactDivide(IntegerType, ValueId, ValueId),
    ExactRemainder(IntegerType, ValueId, ValueId),
    WrappingDivide(IntegerType, ValueId, ValueId),
    WrappingRemainder(IntegerType, ValueId, ValueId),
    SaturatingDivide(IntegerType, ValueId, ValueId),
    SaturatingRemainder(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentCompatiblePolicyScalarExpressionKey {
    ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    Add(IntegerType, ValueId, ValueId),
    Subtract(IntegerType, ValueId, ValueId),
    Multiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IndependentScalarExpressionKey {
    ObligationFree(IndependentTotalScalarExpressionKey),
    ProofCertified(IndependentProofScalarExpressionKey),
    CompatiblePolicy(IndependentCompatiblePolicyScalarExpressionKey),
}

pub(crate) fn independent_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

pub(crate) fn independent_total_scalar_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<(
    IndependentTotalScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
)> {
    let operand_integer = |value: ValueId| match value_types.get(&value) {
        Some(ScalarType::Integer(row)) => Some(*row),
        _ => None,
    };
    Some(match operation {
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => (
            IndependentTotalScalarExpressionKey::BooleanConstant(*value),
            *psi_operation,
            *result,
            ScalarType::Boolean,
        ),
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => (
            IndependentTotalScalarExpressionKey::IntegerConstant(*scalar_type, *value),
            *psi_operation,
            *result,
            *scalar_type,
        ),
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::BooleanNot(*operand),
            *psi_operation,
            *result,
            ScalarType::Boolean,
        ),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::BooleanEqual(left, right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerEqual(scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            (
                IndependentTotalScalarExpressionKey::IntegerLessThan(scalar_type, *left, *right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let scalar_type = operand_integer(*left)?;
            if operand_integer(*right)? != scalar_type {
                return None;
            }
            (
                IndependentTotalScalarExpressionKey::IntegerLessOrEqual(scalar_type, *left, *right),
                *psi_operation,
                *result,
                ScalarType::Boolean,
            )
        }
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::IntegerBitwiseNot(*scalar_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            IndependentTotalScalarExpressionKey::IntegerWiden(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseAnd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseOr(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::IntegerBitwiseXor(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentTotalScalarExpressionKey::WrappingShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentTotalScalarExpressionKey::WrappingShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::WrappingAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentTotalScalarExpressionKey::WrappingSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::WrappingMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::SaturatingAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentTotalScalarExpressionKey::SaturatingSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentTotalScalarExpressionKey::SaturatingMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    })
}

pub(crate) fn independent_proof_scalar_expression(
    operation: &O,
) -> Option<(
    IndependentProofScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    psi_core::ObligationId,
)> {
    Some(match operation {
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => (
            IndependentProofScalarExpressionKey::ExactCast(*source_type, *target_type, *operand),
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            *obligation,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentProofScalarExpressionKey::ExactShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentProofScalarExpressionKey::ExactShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentProofScalarExpressionKey::ExactAdd(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::ExactSubtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentProofScalarExpressionKey::ExactMultiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::ExactDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::ExactRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::WrappingDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::WrappingRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::SaturatingDivide(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentProofScalarExpressionKey::SaturatingRemainder(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        _ => return None,
    })
}

pub(crate) fn independent_compatible_policy_scalar_leader(
    operation: &O,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let row = match operation {
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some((
        IndependentScalarExpressionKey::CompatiblePolicy(row.0),
        row.1,
        row.2,
        row.3,
        None,
    ))
}

pub(crate) fn independent_compatible_policy_scalar_redundant(
    operation: &O,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let row = match operation {
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        _ => return None,
    };
    Some((
        IndependentScalarExpressionKey::CompatiblePolicy(row.0),
        row.1,
        row.2,
        row.3,
        Some(row.4),
    ))
}

pub(crate) fn independent_cse_expression(
    operation: &O,
    value_types: &BTreeMap<ValueId, ScalarType>,
    proof_class: ScalarCseProofClass,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    match proof_class {
        ScalarCseProofClass::ObligationFree => {
            let (key, operation, result, scalar_type) =
                independent_total_scalar_expression(operation, value_types)?;
            Some((
                IndependentScalarExpressionKey::ObligationFree(key),
                operation,
                result,
                scalar_type,
                None,
            ))
        }
        ScalarCseProofClass::ProofCertified => {
            let (key, operation, result, scalar_type, obligation) =
                independent_proof_scalar_expression(operation)?;
            Some((
                IndependentScalarExpressionKey::ProofCertified(key),
                operation,
                result,
                scalar_type,
                Some(obligation),
            ))
        }
        ScalarCseProofClass::CompatiblePolicy => None,
    }
}

pub(crate) fn independently_accepted_operation_fact(
    input: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: OperationId,
    obligation: psi_core::ObligationId,
) -> Option<omega_optimization_core::AcceptedObligationFactIdentity> {
    function
        .facts
        .iter()
        .any(|fact| {
            matches!(
                fact,
                OptimizationFact::OperationObligationReference {
                    obligation: reference,
                    support,
                } if *support == operation && *reference == obligation
            )
        })
        .then(|| {
            input
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == function.machine
                        && fact.operation == operation
                        && fact.obligation == obligation
                })
                .map(|fact| fact.identity)
        })
        .flatten()
}

/// Independently validate and apply one same-block common-subexpression elimination.
pub fn validate_local_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_scalar_common_subexpression_candidate(input, candidate, ScalarCseScope::SameBlock)
}

/// Independently validate and apply one cross-block dominating
/// common-subexpression elimination.
pub fn validate_dominating_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_scalar_common_subexpression_candidate(input, candidate, ScalarCseScope::Dominating)
}

/// Independently validate one obligation-free, proof-certified, or
/// proof-certified compatible-policy scalar
/// expression translated through every incoming binding of an acyclic join.
/// The redundant result identity becomes a new join parameter; every incoming
/// edge supplies the canonical available leader for its translated expression.
pub fn validate_phi_translated_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let proof_class = if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.phi-translated-obligation-free-total-scalar-gvn.v1",
        ) {
        ScalarCseProofClass::ObligationFree
    } else if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.phi-translated-proof-certified-total-scalar-gvn.v1",
        )
    {
        ScalarCseProofClass::ProofCertified
    } else if candidate.rule()
        == OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1",
        )
    {
        ScalarCseProofClass::CompatiblePolicy
    } else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let expected_safety = match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    };
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::ControlFlowGraph)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::Dominators)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != expected_safety
        || !candidate.substitutions().is_empty()
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(patch) = candidate.patch()
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != Some(patch.redundant) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|row| row.machine == patch.redundant.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let join = function
        .blocks
        .iter()
        .find(|row| row.id == patch.redundant.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_index = usize::try_from(patch.redundant.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant = join
        .nodes
        .get(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if function.entry == join.id
        || join.nodes.get(redundant_index + 1).is_none()
        || usize::try_from(patch.parameter_position).ok() != Some(join.parameters.len())
        || join
            .parameters
            .iter()
            .any(|row| row.value == patch.redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let value_types = function
        .parameters
        .iter()
        .map(|row| (row.value, row.scalar_type))
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
        }))
        .chain(function.blocks.iter().flat_map(|block| {
            block.nodes.iter().flat_map(|node| {
                node.definitions
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            })
        }))
        .collect::<BTreeMap<_, _>>();
    let (_, redundant_operation, redundant_result, redundant_type, redundant_obligation) =
        match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_redundant(&redundant.operation)
            }
            _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
        }
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if redundant_operation != patch.redundant_operation
        || redundant_result != patch.redundant_result
        || redundant_type != patch.scalar_type
        || redundant.definitions
            != [ValueDefinition {
                value: redundant_result,
                scalar_type: redundant_type,
                site: ValueDefinitionSite::Node {
                    block: join.id,
                    node: patch.redundant.node,
                },
            }]
        || !redundant.successors.is_empty()
        || !redundant.ownership.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.uses)
            .any(|use_site| use_site.value == redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let _redundant_fact = match (proof_class, redundant_obligation) {
        (ScalarCseProofClass::ObligationFree, None) => {
            if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == redundant_operation)
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        (
            ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy,
            Some(obligation),
        ) => {
            let fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                obligation,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Some(fact)
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    };

    let dominators = independent_reachable_dominators(function);
    let mut expected_incoming = Vec::new();
    for source in &function.blocks {
        for edge in source
            .nodes
            .iter()
            .flat_map(|node| &node.successors)
            .filter(|edge| edge.target == join.id)
        {
            if edge.bindings.len() != join.parameters.len() {
                return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
            }
            let mut translated = redundant.operation.clone();
            for (parameter, binding) in join.parameters.iter().zip(&edge.bindings) {
                if binding.parameter != parameter.value
                    || binding.scalar_type != parameter.scalar_type
                    || value_types.get(&binding.argument) != Some(&binding.scalar_type)
                {
                    return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
                }
                rewrite_scalar_value_uses(&mut translated, parameter.value, binding.argument);
            }
            let (translated_key, _, _, translated_type, _) = match proof_class {
                ScalarCseProofClass::CompatiblePolicy => {
                    independent_compatible_policy_scalar_redundant(&translated)
                }
                _ => independent_cse_expression(&translated, &value_types, proof_class),
            }
            .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
            let mut available_leaders = Vec::new();
            let mut missing_leader_evidence = false;
            for leader_block in &function.blocks {
                for (node_index, node) in leader_block.nodes.iter().enumerate() {
                    let available = if leader_block.id == source.id {
                        node_index + 1 < source.nodes.len()
                    } else {
                        dominators
                            .get(&source.id)
                            .is_some_and(|rows| rows.contains(&leader_block.id))
                    };
                    if !available {
                        continue;
                    }
                    let Some((key, operation, result, scalar_type, obligation)) = (match proof_class
                    {
                        ScalarCseProofClass::CompatiblePolicy => {
                            independent_compatible_policy_scalar_leader(&node.operation)
                        }
                        _ => independent_cse_expression(&node.operation, &value_types, proof_class),
                    }) else {
                        continue;
                    };
                    let admitted = match (proof_class, obligation) {
                        (ScalarCseProofClass::ObligationFree, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        (ScalarCseProofClass::ProofCertified, Some(obligation)) => {
                            independently_accepted_operation_fact(
                                input,
                                function,
                                operation,
                                obligation,
                            )
                            .is_some()
                        }
                        (ScalarCseProofClass::CompatiblePolicy, None) => !function
                            .facts
                            .iter()
                            .any(|fact| matches!(fact, OptimizationFact::OperationObligationReference { support, .. } if *support == operation)),
                        _ => false,
                    };
                    if !admitted
                        && ((proof_class == ScalarCseProofClass::ProofCertified
                            && obligation.is_some())
                            || proof_class == ScalarCseProofClass::CompatiblePolicy)
                        && key == translated_key
                        && scalar_type == translated_type
                    {
                        missing_leader_evidence = true;
                    }
                    if admitted && key == translated_key && scalar_type == translated_type {
                        available_leaders.push((
                            NodeLocation {
                                machine: function.machine,
                                block: leader_block.id,
                                node: u32::try_from(node_index).map_err(|_| {
                                    OptimizationUnitValidationError::CandidateLocationMissing
                                })?,
                            },
                            operation,
                            result,
                            obligation,
                        ));
                    }
                }
            }
            let canonical = available_leaders
                .into_iter()
                .min_by_key(|(location, _, _, _)| {
                    (
                        dominators
                            .get(&location.block)
                            .map_or(usize::MAX, BTreeSet::len),
                        *location,
                    )
                })
                .ok_or(if missing_leader_evidence {
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch
                } else {
                    OptimizationUnitValidationError::CandidatePatchMismatch
                })?;
            expected_incoming.push(PhiTranslatedScalarIncoming {
                source: source.id,
                edge: edge.psi_edge,
                leader: canonical.0,
                leader_operation: canonical.1,
                leader_result: canonical.2,
            });
        }
    }
    expected_incoming.sort_by_key(|row| (row.edge, row.source));
    if expected_incoming.len() < 2 || patch.incoming != expected_incoming {
        return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
    }
    let (expected_blocks, accepted_provenance) =
        reconstruct_phi_translated_cse_accounting(function, &patch)
            .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|row| row.machine == patch.redundant.machine)
        .expect("candidate function exists");
    let output_join = output_function
        .blocks
        .iter_mut()
        .find(|row| row.id == patch.redundant.block)
        .expect("candidate join exists");
    output_join.parameters.push(ValueDefinition {
        value: patch.redundant_result,
        scalar_type: patch.scalar_type,
        site: ValueDefinitionSite::BlockParameter {
            block: patch.redundant.block,
            position: patch.parameter_position,
        },
    });
    let removed = output_join.nodes.remove(redundant_index);
    let receiver = output_join
        .nodes
        .get_mut(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    for incoming in &patch.incoming {
        let source = output_function
            .blocks
            .iter_mut()
            .find(|row| row.id == incoming.source)
            .expect("incoming source exists");
        let node = source
            .nodes
            .iter_mut()
            .find(|node| {
                node.successors
                    .iter()
                    .any(|edge| edge.psi_edge == incoming.edge)
            })
            .expect("incoming edge exists");
        let edge = node
            .successors
            .iter()
            .find(|edge| edge.psi_edge == incoming.edge)
            .expect("incoming edge exists");
        let mut bindings = edge.bindings.clone();
        bindings.push(omega_abstract_operations::ValueBinding {
            parameter: patch.redundant_result,
            argument: incoming.leader_result,
            scalar_type: patch.scalar_type,
        });
        if !rewrite_successor_operation(
            &mut node.operation,
            incoming.edge,
            patch.redundant.block,
            &bindings,
        ) {
            return Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|row| row.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|row| row.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(match proof_class {
            ScalarCseProofClass::ObligationFree => {
                b"omega.validator.phi-translated-obligation-free-total-scalar-gvn.v1"
            }
            ScalarCseProofClass::ProofCertified => {
                b"omega.validator.phi-translated-proof-certified-total-scalar-gvn.v1"
            }
            ScalarCseProofClass::CompatiblePolicy => {
                b"omega.validator.phi-translated-proof-certified-compatible-policy-scalar-gvn.v1"
            }
        }),
        provenance: accepted_provenance,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarCseScope {
    SameBlock,
    Dominating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarCseProofClass {
    ObligationFree,
    ProofCertified,
    CompatiblePolicy,
}

pub(crate) fn validate_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    scope: ScalarCseScope,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    let proof_class = match (scope, candidate.rule()) {
        (ScalarCseScope::SameBlock, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.same-block-obligation-free-total-scalar-cse.v1",
                ) =>
        {
            ScalarCseProofClass::ObligationFree
        }
        (ScalarCseScope::Dominating, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dominator-obligation-free-total-scalar-gvn.v1",
                ) =>
        {
            ScalarCseProofClass::ObligationFree
        }
        (ScalarCseScope::SameBlock, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.same-block-proof-certified-total-scalar-cse.v1",
                ) =>
        {
            ScalarCseProofClass::ProofCertified
        }
        (ScalarCseScope::Dominating, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dominator-proof-certified-total-scalar-gvn.v1",
                ) =>
        {
            ScalarCseProofClass::ProofCertified
        }
        (ScalarCseScope::SameBlock, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.same-block-proof-certified-compatible-policy-scalar-cse.v1",
                ) =>
        {
            ScalarCseProofClass::CompatiblePolicy
        }
        (ScalarCseScope::Dominating, rule)
            if rule
                == OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.dominator-proof-certified-compatible-policy-scalar-gvn.v1",
                ) =>
        {
            ScalarCseProofClass::CompatiblePolicy
        }
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let expected_safety = match proof_class {
        ScalarCseProofClass::ObligationFree => OptimizationSafetyClass::ExactOperationSemantics,
        ScalarCseProofClass::ProofCertified | ScalarCseProofClass::CompatiblePolicy => {
            OptimizationSafetyClass::ProofCertified
        }
    };
    if !candidate
        .required_analyses()
        .contains(AnalysisKind::UseDefinition)
        || !candidate
            .required_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::UseDefinition)
        || !candidate
            .invalidated_analyses()
            .contains(AnalysisKind::EffectSummaries)
        || candidate.safety_class() != expected_safety
        || (scope == ScalarCseScope::Dominating
            && (!candidate
                .required_analyses()
                .contains(AnalysisKind::ControlFlowGraph)
                || !candidate
                    .required_analyses()
                    .contains(AnalysisKind::Dominators)))
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    let patch = match (scope, candidate.patch()) {
        (
            ScalarCseScope::SameBlock,
            PsiRewritePatch::EliminateLocalScalarCommonSubexpression(patch),
        ) => patch,
        (
            ScalarCseScope::Dominating,
            PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(patch),
        ) => LocalScalarCommonSubexpressionRewrite {
            leader: patch.leader,
            redundant: patch.redundant,
            leader_operation: patch.leader_operation,
            redundant_operation: patch.redundant_operation,
            leader_result: patch.leader_result,
            redundant_result: patch.redundant_result,
            scalar_type: patch.scalar_type,
        },
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if candidate.node_decision_point() != Some(patch.redundant) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let expected_substitution = [ScalarSubstitution {
        from: patch.redundant_result,
        to: patch.leader_result,
        scalar_type: patch.scalar_type,
    }];
    if candidate.substitutions() != expected_substitution {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.leader.machine != patch.redundant.machine {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|row| row.machine == patch.leader.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let leader_block = function
        .blocks
        .iter()
        .find(|row| row.id == patch.leader.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_block = function
        .blocks
        .iter()
        .find(|row| row.id == patch.redundant.block)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let leader = leader_block
        .nodes
        .get(
            usize::try_from(patch.leader.node)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
        )
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant_index = usize::try_from(patch.redundant.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let redundant = redundant_block
        .nodes
        .get(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    if redundant_block.nodes.get(redundant_index + 1).is_none() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    match scope {
        ScalarCseScope::SameBlock
            if patch.leader.block != patch.redundant.block
                || patch.leader.node >= patch.redundant.node =>
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        ScalarCseScope::Dominating if patch.leader.block == patch.redundant.block => {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        _ => {}
    }
    let value_types = function
        .parameters
        .iter()
        .map(|row| (row.value, row.scalar_type))
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .map(|row| (row.value, row.scalar_type))
        }))
        .chain(function.blocks.iter().flat_map(|block| {
            block.nodes.iter().flat_map(|node| {
                node.definitions
                    .iter()
                    .map(|row| (row.value, row.scalar_type))
            })
        }))
        .collect::<BTreeMap<_, _>>();
    let admitted_expression = |operation: &O| {
        let row = match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_leader(operation)?
            }
            _ => independent_cse_expression(operation, &value_types, proof_class)?,
        };
        match (proof_class, row.4) {
            (ScalarCseProofClass::ObligationFree, None) => Some(row),
            (ScalarCseProofClass::ProofCertified, Some(obligation))
                if independently_accepted_operation_fact(input, function, row.1, obligation)
                    .is_some() =>
            {
                Some(row)
            }
            (ScalarCseProofClass::CompatiblePolicy, None)
                if !function.facts.iter().any(|fact| {
                    matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                        if *support == row.1)
                }) =>
            {
                Some(row)
            }
            _ => None,
        }
    };
    let (leader_key, leader_operation, leader_result, leader_type, leader_obligation) =
        match proof_class {
            ScalarCseProofClass::CompatiblePolicy => {
                independent_compatible_policy_scalar_leader(&leader.operation)
            }
            _ => independent_cse_expression(&leader.operation, &value_types, proof_class),
        }
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    let (
        redundant_key,
        redundant_operation,
        redundant_result,
        redundant_type,
        redundant_obligation,
    ) = match proof_class {
        ScalarCseProofClass::CompatiblePolicy => {
            independent_compatible_policy_scalar_redundant(&redundant.operation)
        }
        _ => independent_cse_expression(&redundant.operation, &value_types, proof_class),
    }
    .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if leader_key != redundant_key
        || leader_operation != patch.leader_operation
        || redundant_operation != patch.redundant_operation
        || leader_result != patch.leader_result
        || redundant_result != patch.redundant_result
        || leader_type != patch.scalar_type
        || redundant_type != patch.scalar_type
        || leader_result == redundant_result
        || leader_operation == redundant_operation
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let _proof_facts = match (proof_class, leader_obligation, redundant_obligation) {
        (ScalarCseProofClass::ObligationFree, None, None) => {
            if candidate.accepted_obligation_witness().is_some()
                || function.facts.iter().any(|fact| {
                    matches!(
                        fact,
                        OptimizationFact::OperationObligationReference { support, .. }
                            if *support == leader_operation || *support == redundant_operation
                    )
                })
            {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        (ScalarCseProofClass::ProofCertified, Some(leader), Some(redundant)) => {
            let leader_fact =
                independently_accepted_operation_fact(input, function, leader_operation, leader)
                    .ok_or(
                        OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                    )?;
            let redundant_fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                redundant,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            Some((leader_fact, redundant_fact))
        }
        (ScalarCseProofClass::CompatiblePolicy, None, Some(redundant)) => {
            if function.facts.iter().any(|fact| {
                matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                    if *support == leader_operation)
            }) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            let redundant_fact = independently_accepted_operation_fact(
                input,
                function,
                redundant_operation,
                redundant,
            )
            .ok_or(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)?;
            if candidate.accepted_obligation_witness() != Some(redundant_fact) {
                return Err(
                    OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch,
                );
            }
            None
        }
        _ => {
            return Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch);
        }
    };
    if scope == ScalarCseScope::SameBlock {
        let canonical_leader = leader_block
            .nodes
            .iter()
            .take(redundant_index)
            .enumerate()
            .filter_map(|(node, candidate)| {
                let (key, _, _, scalar_type, _) = admitted_expression(&candidate.operation)?;
                (key == redundant_key && scalar_type == patch.scalar_type).then_some(NodeLocation {
                    machine: function.machine,
                    block: leader_block.id,
                    node: u32::try_from(node).ok()?,
                })
            })
            .next();
        if canonical_leader != Some(patch.leader) {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    }
    if scope == ScalarCseScope::Dominating {
        let dominators = independent_reachable_dominators(function);
        if !dominators
            .get(&patch.redundant.block)
            .is_some_and(|rows| rows.contains(&patch.leader.block))
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
        let canonical_leader = function
            .blocks
            .iter()
            .filter(|block| block.id != patch.redundant.block)
            .filter(|block| {
                dominators
                    .get(&patch.redundant.block)
                    .is_some_and(|rows| rows.contains(&block.id))
            })
            .flat_map(|block| {
                block
                    .nodes
                    .iter()
                    .enumerate()
                    .filter_map(|(node, candidate)| {
                        let (key, _, _, scalar_type, _) =
                            admitted_expression(&candidate.operation)?;
                        (key == redundant_key && scalar_type == patch.scalar_type).then_some(
                            NodeLocation {
                                machine: function.machine,
                                block: block.id,
                                node: u32::try_from(node).ok()?,
                            },
                        )
                    })
            })
            .min_by_key(|location| {
                (
                    dominators
                        .get(&location.block)
                        .map_or(usize::MAX, BTreeSet::len),
                    *location,
                )
            });
        if canonical_leader != Some(patch.leader)
            || function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.uses)
                .filter(|use_site| use_site.value == redundant_result)
                .any(|use_site| {
                    if use_site.block == patch.leader.block {
                        patch.leader.node >= use_site.node
                    } else {
                        !dominators
                            .get(&use_site.block)
                            .is_some_and(|rows| rows.contains(&patch.leader.block))
                    }
                })
        {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    }
    if leader.definitions
        != [ValueDefinition {
            value: leader_result,
            scalar_type: leader_type,
            site: ValueDefinitionSite::Node {
                block: leader_block.id,
                node: patch.leader.node,
            },
        }]
        || redundant.definitions
            != [ValueDefinition {
                value: redundant_result,
                scalar_type: redundant_type,
                site: ValueDefinitionSite::Node {
                    block: redundant_block.id,
                    node: patch.redundant.node,
                },
            }]
        || !leader.successors.is_empty()
        || !redundant.successors.is_empty()
        || !leader.ownership.is_empty()
        || !redundant.ownership.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|row| &row.nodes)
            .flat_map(|row| &row.uses)
            .any(|row| row.value == redundant_result)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let (expected_blocks, accepted_provenance) = reconstruct_local_cse_accounting(function, patch)
        .ok_or(OptimizationUnitValidationError::CandidateProvenanceMismatch)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != accepted_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|row| row.machine == patch.leader.machine)
        .expect("candidate function exists");
    let output_block = output_function
        .blocks
        .iter_mut()
        .find(|row| row.id == patch.redundant.block)
        .expect("candidate block exists");
    let removed = output_block.nodes.remove(redundant_index);
    let receiver = output_block
        .nodes
        .get_mut(redundant_index)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    receiver.provenance.extend_from_slice(&removed.provenance);
    receiver.fuel.extend_from_slice(&removed.fuel);
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            rewrite_scalar_value_uses(&mut node.operation, redundant_result, leader_result);
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = effect
                .checked_add(1)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|row| row.machine == function.machine)
        .expect("output function exists");
    for input_block in &function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|row| row.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            match (scope, proof_class) {
                (ScalarCseScope::SameBlock, ScalarCseProofClass::ObligationFree) => {
                    b"omega.validator.same-block-obligation-free-total-scalar-cse.v1"
                }
                (ScalarCseScope::Dominating, ScalarCseProofClass::ObligationFree) => {
                    b"omega.validator.dominator-total-scalar-cse.v1"
                }
                (ScalarCseScope::SameBlock, ScalarCseProofClass::ProofCertified) => {
                    b"omega.validator.same-block-proof-certified-total-scalar-cse.v1"
                }
                (ScalarCseScope::Dominating, ScalarCseProofClass::ProofCertified) => {
                    b"omega.validator.dominator-proof-certified-total-scalar-gvn.v1"
                }
                (ScalarCseScope::SameBlock, ScalarCseProofClass::CompatiblePolicy) => {
                    b"omega.validator.same-block-proof-certified-compatible-policy-scalar-cse.v1"
                }
                (ScalarCseScope::Dominating, ScalarCseProofClass::CompatiblePolicy) => {
                    b"omega.validator.dominator-proof-certified-compatible-policy-scalar-gvn.v1"
                }
            },
        ),
        provenance: accepted_provenance,
    })
}

pub(crate) fn independent_reachable_dominators(
    function: &PsiOptimizationFunction,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .map(|node| node.successors.iter().map(|edge| edge.target).collect())
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
    let mut reachable = BTreeSet::from([function.entry]);
    let mut frontier = vec![function.entry];
    while let Some(block) = frontier.pop() {
        for successor in successors.get(&block).into_iter().flatten() {
            if reachable.insert(*successor) {
                frontier.push(*successor);
            }
        }
    }
    let mut predecessors = reachable
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in &successors {
        if !reachable.contains(source) {
            continue;
        }
        for target in targets.iter().filter(|target| reachable.contains(target)) {
            predecessors.get_mut(target).unwrap().insert(*source);
        }
    }
    let mut result = reachable
        .iter()
        .copied()
        .map(|block| {
            (
                block,
                if block == function.entry {
                    BTreeSet::from([block])
                } else {
                    reachable.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in reachable
            .iter()
            .copied()
            .filter(|block| *block != function.entry)
        {
            let mut incoming = predecessors[&block].iter();
            let mut next = incoming
                .next()
                .map(|predecessor| result[predecessor].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

pub(crate) fn independently_replacement_dominates_uses(
    function: &PsiOptimizationFunction,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    replacement: ValueId,
    parameter: ValueId,
    scalar_type: ScalarType,
) -> bool {
    if replacement == parameter {
        return false;
    }
    let Some(definition) = function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == replacement)
    else {
        return false;
    };
    if definition.scalar_type != scalar_type {
        return false;
    }
    function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| &node.uses))
        .filter(|use_site| use_site.value == parameter)
        .all(|use_site| match definition.site {
            ValueDefinitionSite::FunctionParameter(_) => true,
            ValueDefinitionSite::BlockParameter {
                block: defining, ..
            } => dominators
                .get(&use_site.block)
                .is_some_and(|rows| rows.contains(&defining)),
            ValueDefinitionSite::Node {
                block: defining,
                node,
            } if defining == use_site.block => node < use_site.node,
            ValueDefinitionSite::Node {
                block: defining, ..
            } => dominators
                .get(&use_site.block)
                .is_some_and(|rows| rows.contains(&defining)),
        })
}
