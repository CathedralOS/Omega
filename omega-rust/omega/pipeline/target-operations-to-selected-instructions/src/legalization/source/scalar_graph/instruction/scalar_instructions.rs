//! Scalar arithmetic, logic, comparison and cast operations projected to
//! the legalized instruction kind that realizes each.

use super::super::{
    AbstractOperation, Error, LegalizedExactIntegerOperator, LegalizedScalarComparison,
    LegalizedScalarInstructionKind, PsiOptimizationUnit,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use semantic_vocabulary::ScalarType;

pub(super) fn project_integer_exact_cast(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::IntegerExactCast {
        psi_operation,
        operand,
        source_type,
        obligation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_integer_exact_cast")
    };
    let kind = {
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
        if !optimized.facts.iter().any(|fact| matches!(fact,
        optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
        if referenced == obligation && support == psi_operation)) {
        return Err(Error::SourceCustodyMismatch);
    }
        LegalizedScalarInstructionKind::IntegerExactCast {
            operand: *operand,
            source_type: *source_type,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

/// Saturating add/subtract select the legalized kind from the carrier: the
/// unsigned 64-bit and signed 32-bit families are realized; every other
/// width was already rejected as an unsupported family by node admission.
pub(super) fn project_saturating_integer_add_or_subtract(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (adds, scalar_type, left, right) = match &node.operation {
        AbstractOperation::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        } => (true, *scalar_type, *left, *right),
        AbstractOperation::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        } => (false, *scalar_type, *left, *right),
        _ => unreachable!("dispatched project_saturating_integer_add_or_subtract"),
    };
    if [left, right].iter().any(|value| {
        scalar_graph_input::value_type(optimized, *value) != Some(ScalarType::Integer(scalar_type))
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(
        match (
            scalar_type == scalar_graph_input::u64_type(),
            scalar_graph_input::supports_signed_saturating_i32(scalar_type),
            adds,
        ) {
            (true, _, true) => LegalizedScalarInstructionKind::SaturatingAddU64 { left, right },
            (true, _, false) => {
                LegalizedScalarInstructionKind::SaturatingSubtractU64 { left, right }
            }
            (_, true, true) => LegalizedScalarInstructionKind::SaturatingAddI32 { left, right },
            (_, true, false) => {
                LegalizedScalarInstructionKind::SaturatingSubtractI32 { left, right }
            }
            _ => return Err(Error::SourceCustodyMismatch),
        },
    )
}

pub(super) fn project_saturating_integer_divide(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::SaturatingIntegerDivide {
        psi_operation,
        obligation,
        scalar_type,
        left,
        right,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_saturating_integer_divide")
    };
    if !scalar_graph_input::supports_signed_saturating_i32(*scalar_type)
        || [left, right].iter().any(|value| {
            scalar_graph_input::value_type(optimized, **value)
                != Some(ScalarType::Integer(*scalar_type))
        })
    {
        return Err(Error::SourceCustodyMismatch);
    }
    // Saturating clamps MIN / -1 to MAX; it does not define division by
    // zero, so the accepted nonzero-divisor fact stays with the instruction.
    let accepted_fact =
        accepted_nonzero_divisor_fact(optimized, unit, *psi_operation, *obligation)?;
    Ok(LegalizedScalarInstructionKind::SaturatingDivideI32 {
        left: *left,
        right: *right,
        obligation: *obligation,
        accepted_fact,
    })
}

/// The single accepted fact discharging this operation's divisor obligation,
/// which the optimized function must also reference from the operation.
fn accepted_nonzero_divisor_fact(
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    psi_operation: semantic_vocabulary::OperationId,
    obligation: semantic_vocabulary::ObligationId,
) -> Result<optimization_core::AcceptedObligationFactIdentity, LegalizationError> {
    let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
        fact.machine == optimized.machine
            && fact.operation == psi_operation
            && fact.obligation == obligation
    });
    let fact = facts.next().ok_or(Error::SourceCustodyMismatch)?;
    if facts.next().is_some()
        || !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if *referenced == obligation && *support == psi_operation))
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(fact.identity)
}

pub(super) fn project_wrapping_integer_remainder(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::WrappingIntegerRemainder {
        psi_operation,
        obligation,
        scalar_type,
        left,
        right,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_wrapping_integer_remainder")
    };
    let kind = {
        if !scalar_graph_input::supports_signed_wrapping_remainder(*scalar_type)
            || [left, right].iter().any(|value| {
                scalar_graph_input::value_type(optimized, **value)
                    != Some(ScalarType::Integer(*scalar_type))
            })
        {
            return Err(Error::SourceCustodyMismatch);
        }
        // Wrapping defines MIN % -1 as zero, not a failed Exact quotient.
        // It does not define division by zero: retain that accepted fact.
        let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
            fact.machine == optimized.machine
                && fact.operation == *psi_operation
                && fact.obligation == *obligation
        });
        let fact = facts.next().ok_or(Error::SourceCustodyMismatch)?;
        if facts.next().is_some()
        || !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if referenced == obligation && support == psi_operation))
    {
        return Err(Error::SourceCustodyMismatch);
    }
        LegalizedScalarInstructionKind::WrappingRemainder {
            left: *left,
            right: *right,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_exact_integer_add(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (AbstractOperation::ExactIntegerAdd {
        psi_operation,
        obligation,
        left,
        right,
        ..
    }
    | AbstractOperation::ExactIntegerSubtract {
        psi_operation,
        obligation,
        left,
        right,
        ..
    }
    | AbstractOperation::ExactIntegerDivide {
        psi_operation,
        obligation,
        left,
        right,
        ..
    }) = &node.operation
    else {
        unreachable!("dispatched project_exact_integer_add")
    };
    let kind = {
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *obligation
            })
            .ok_or(Error::SourceCustodyMismatch)?;
        if !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if referenced == obligation && support == psi_operation)) {
                return Err(Error::SourceCustodyMismatch);
            }
        LegalizedScalarInstructionKind::ExactBinary {
            operator: if matches!(node.operation, AbstractOperation::ExactIntegerAdd { .. }) {
                LegalizedExactIntegerOperator::Add
            } else if matches!(node.operation, AbstractOperation::ExactIntegerDivide { .. }) {
                LegalizedExactIntegerOperator::Divide
            } else {
                LegalizedExactIntegerOperator::Subtract
            },
            left: *left,
            right: *right,
            obligation: *obligation,
            accepted_fact: fact.identity,
        }
    };
    Ok(kind)
}

pub(super) fn project_boolean_equal(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (AbstractOperation::BooleanEqual { left, right, .. }
    | AbstractOperation::IntegerEqual { left, right, .. }
    | AbstractOperation::IntegerLessThan { left, right, .. }
    | AbstractOperation::IntegerLessOrEqual { left, right, .. }) = &node.operation
    else {
        unreachable!("dispatched project_boolean_equal")
    };
    let kind = {
        let predicate = match node.operation {
            AbstractOperation::BooleanEqual { .. } | AbstractOperation::IntegerEqual { .. } => {
                LegalizedScalarComparison::Equal
            }
            AbstractOperation::IntegerLessThan { .. } => LegalizedScalarComparison::LessThan,
            AbstractOperation::IntegerLessOrEqual { .. } => LegalizedScalarComparison::LessOrEqual,
            _ => return Err(Error::SourceCustodyMismatch),
        };
        let operand_type =
            scalar_graph_input::value_type(optimized, *left).ok_or(Error::SourceCustodyMismatch)?;
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            left: *left,
            right: *right,
        }
    };
    Ok(kind)
}
