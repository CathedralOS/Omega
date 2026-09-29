//! Scalar arithmetic, logic, comparison, cast and constant instructions
//! replayed against the abstract operation each legalizes.

use super::super::{Error, PsiOptimizationUnit};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::{
    AbstractOperation, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftPolicy,
};
use legalized_operations::{
    LegalizedExactIntegerOperator, LegalizedScalarComparison, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind,
};
use semantic_vocabulary::ScalarType;

pub(super) fn validate_integer_exact_cast(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::IntegerExactCast {
            operand,
            source_type,
            obligation,
            accepted_fact,
        },
        AbstractOperation::IntegerExactCast {
            psi_operation,
            operand: source,
            source_type: source_integer,
            obligation: source_obligation,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_integer_exact_cast")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == optimized.machine
                && fact.operation == *psi_operation
                && fact.obligation == *source_obligation
        })
        .ok_or(Error::custody())?;
    if operand != source || source_type != source_integer || obligation != source_obligation || *accepted_fact != fact.identity
        || !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if referenced == source_obligation && support == psi_operation)) {
        return Err(invalid);
    }
    Ok(())
}

/// Wrapping divide and remainder replay every fixed native carrier the
/// source operation declares, with the exact accepted nonzero-divisor fact:
/// wrapping defines MIN / -1 and MIN % -1, but not division by zero.
pub(super) fn validate_wrapping_division(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::WrappingRemainder {
            left,
            right,
            obligation,
            accepted_fact,
        }
        | LegalizedScalarInstructionKind::WrappingDivide {
            left,
            right,
            obligation,
            accepted_fact,
        },
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            scalar_type,
            arithmetic,
            left: source_left,
            right: source_right,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_wrapping_division")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let Some(source_obligation) = arithmetic.obligation() else {
        return Err(invalid);
    };
    let source_obligation = &source_obligation;
    let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
        fact.machine == optimized.machine
            && fact.operation == *psi_operation
            && fact.obligation == *source_obligation
    });
    let fact = facts.next().ok_or(invalid.clone())?;
    if facts.next().is_some()
        || !scalar_graph_input::supports_wrapping_division(*scalar_type)
        || [source_left, source_right].iter().any(|value| {
            scalar_graph_input::value_type(optimized, **value)
                != Some(ScalarType::Integer(*scalar_type))
        })
        || left != source_left
        || right != source_right
        || obligation != source_obligation
        || *accepted_fact != fact.identity
        || !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if referenced == source_obligation && support == psi_operation))
    {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_exact_binary(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::ExactBinary {
            operator,
            left,
            right,
            obligation,
            accepted_fact,
        },
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            arithmetic,
            left: source_left,
            right: source_right,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_exact_binary")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    if arithmetic.policy() != IntegerOverflowPolicy::Exact {
        return Err(invalid);
    }
    let expected_operator = match arithmetic.operator() {
        IntegerArithmeticOperator::Add => LegalizedExactIntegerOperator::Add,
        IntegerArithmeticOperator::Subtract => LegalizedExactIntegerOperator::Subtract,
        IntegerArithmeticOperator::Divide => LegalizedExactIntegerOperator::Divide,
        IntegerArithmeticOperator::Remainder => LegalizedExactIntegerOperator::Remainder,
        IntegerArithmeticOperator::Multiply => LegalizedExactIntegerOperator::Multiply,
    };
    let Some(source_obligation) = arithmetic.obligation() else {
        return Err(invalid);
    };
    let source_obligation = &source_obligation;
    let fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == optimized.machine
                && fact.operation == *psi_operation
                && fact.obligation == *source_obligation
        })
        .ok_or(Error::custody())?;
    if *operator != expected_operator || left != source_left || right != source_right
            || obligation != source_obligation || *accepted_fact != fact.identity
            || !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if referenced == source_obligation && support == psi_operation)) {
            return Err(invalid);
        }
    Ok(())
}

/// The shift replay checks the independently typed count next to the
/// shifted value; the exact forms additionally replay the accepted in-range
/// fact custody every proof-bearing operation keeps.
pub(super) fn validate_shift(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (
        value,
        count,
        exact_custody,
        psi_operation,
        source_value,
        source_count,
        value_type,
        count_type,
    ) = match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
            | LegalizedScalarInstructionKind::WrappingShiftRight { value, count },
            AbstractOperation::IntegerShift {
                psi_operation,
                value_type,
                count_type,
                policy: IntegerShiftPolicy::Wrapping,
                value: source_value,
                count: source_count,
                ..
            },
        ) => (
            value,
            count,
            None,
            psi_operation,
            source_value,
            source_count,
            value_type,
            count_type,
        ),
        (
            LegalizedScalarInstructionKind::ExactShiftLeft {
                value,
                count,
                obligation,
                accepted_fact,
            }
            | LegalizedScalarInstructionKind::ExactShiftRight {
                value,
                count,
                obligation,
                accepted_fact,
            },
            AbstractOperation::IntegerShift {
                psi_operation,
                value_type,
                count_type,
                policy:
                    IntegerShiftPolicy::Exact {
                        obligation: source_obligation,
                    },
                value: source_value,
                count: source_count,
                ..
            },
        ) => (
            value,
            count,
            Some((obligation, accepted_fact, source_obligation)),
            psi_operation,
            source_value,
            source_count,
            value_type,
            count_type,
        ),
        _ => unreachable!("dispatched validate_shift"),
    };
    if scalar_graph_input::scalar_shape(ScalarType::Integer(*value_type)).is_none()
        || scalar_graph_input::scalar_shape(ScalarType::Integer(*count_type)).is_none()
        || value != source_value
        || count != source_count
        || scalar_graph_input::value_type(optimized, *value)
            != Some(ScalarType::Integer(*value_type))
        || scalar_graph_input::value_type(optimized, *count)
            != Some(ScalarType::Integer(*count_type))
    {
        return Err(invalid);
    }
    if let Some((obligation, accepted_fact, source_obligation)) = exact_custody {
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *source_obligation
            })
            .ok_or(Error::custody())?;
        if obligation != source_obligation
            || *accepted_fact != fact.identity
            || !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if referenced == source_obligation && support == psi_operation))
        {
            return Err(invalid);
        }
    }
    Ok(())
}

pub(super) fn validate_compare(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let (
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            right_operand_type,
            left,
            right,
        },
        AbstractOperation::BooleanEqual {
            left: source_left,
            right: source_right,
            ..
        }
        | AbstractOperation::IntegerEqual {
            left: source_left,
            right: source_right,
            ..
        }
        | AbstractOperation::IntegerLessThan {
            left: source_left,
            right: source_right,
            ..
        }
        | AbstractOperation::IntegerLessOrEqual {
            left: source_left,
            right: source_right,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_compare")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let expected = match node.operation {
        AbstractOperation::BooleanEqual { .. } | AbstractOperation::IntegerEqual { .. } => {
            LegalizedScalarComparison::Equal
        }
        AbstractOperation::IntegerLessThan { .. } => LegalizedScalarComparison::LessThan,
        AbstractOperation::IntegerLessOrEqual { .. } => LegalizedScalarComparison::LessOrEqual,
        _ => return Err(invalid),
    };
    if *predicate != expected
        || left != source_left
        || right != source_right
        || scalar_graph_input::value_type(optimized, *source_left) != Some(*operand_type)
        || scalar_graph_input::value_type(optimized, *source_right) != Some(*right_operand_type)
    {
        return Err(invalid);
    }
    Ok(())
}
