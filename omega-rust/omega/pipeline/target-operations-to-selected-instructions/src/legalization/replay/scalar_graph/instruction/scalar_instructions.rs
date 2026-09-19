//! Scalar arithmetic, logic, comparison, cast and constant instructions
//! replayed against the abstract operation each legalizes.

use super::super::{
    AbstractOperation, Error, LegalizedExactIntegerOperator, LegalizedScalarComparison,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, PsiOptimizationUnit,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
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
        .ok_or(Error::SourceCustodyMismatch)?;
    if operand != source || source_type != source_integer || obligation != source_obligation || *accepted_fact != fact.identity
        || !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if referenced == source_obligation && support == psi_operation)) {
        return Err(invalid);
    }
    Ok(())
}

pub(super) fn validate_wrapping_remainder(
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
        },
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation: source_obligation,
            scalar_type,
            left: source_left,
            right: source_right,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_wrapping_remainder")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
        fact.machine == optimized.machine
            && fact.operation == *psi_operation
            && fact.obligation == *source_obligation
    });
    let fact = facts.next().ok_or(invalid.clone())?;
    if facts.next().is_some()
        || !scalar_graph_input::supports_signed_wrapping_remainder(*scalar_type)
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
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation: source_obligation,
            left: source_left,
            right: source_right,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation: source_obligation,
            left: source_left,
            right: source_right,
            ..
        }
        | AbstractOperation::ExactIntegerDivide {
            psi_operation,
            obligation: source_obligation,
            left: source_left,
            right: source_right,
            ..
        }
        | AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            obligation: source_obligation,
            left: source_left,
            right: source_right,
            ..
        },
    ) = (&actual.kind, &node.operation)
    else {
        unreachable!("dispatched validate_exact_binary")
    };
    let invalid = Error::NonCanonicalLegalizedPlan;
    let expected_operator = match node.operation {
        AbstractOperation::ExactIntegerAdd { .. } => LegalizedExactIntegerOperator::Add,
        AbstractOperation::ExactIntegerSubtract { .. } => LegalizedExactIntegerOperator::Subtract,
        AbstractOperation::ExactIntegerDivide { .. } => LegalizedExactIntegerOperator::Divide,
        AbstractOperation::ExactIntegerMultiply { .. } => LegalizedExactIntegerOperator::Multiply,
        _ => return Err(invalid),
    };
    let fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == optimized.machine
                && fact.operation == *psi_operation
                && fact.obligation == *source_obligation
        })
        .ok_or(Error::SourceCustodyMismatch)?;
    if *operator != expected_operator || left != source_left || right != source_right
            || obligation != source_obligation || *accepted_fact != fact.identity
            || !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if referenced == source_obligation && support == psi_operation)) {
            return Err(invalid);
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
    {
        return Err(invalid);
    }
    Ok(())
}
