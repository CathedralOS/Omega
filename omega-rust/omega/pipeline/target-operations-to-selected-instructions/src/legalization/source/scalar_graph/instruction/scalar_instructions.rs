//! Scalar arithmetic, logic, comparison and cast operations projected to
//! the legalized instruction kind that realizes each.

use super::super::{Error, PsiOptimizationUnit};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use legalized_operations::{
    LegalizedExactIntegerOperator, LegalizedScalarComparison, LegalizedScalarInstructionKind,
};
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
            .ok_or(Error::custody())?;
        if !optimized.facts.iter().any(|fact| matches!(fact,
        optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
        if referenced == obligation && support == psi_operation)) {
        return Err(Error::custody());
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

/// Saturating add, subtract, and multiply name the carrier the source
/// operation declares and carry no obligation: saturation defines every
/// result. Node admission already rejected every non-carrier width as an
/// unsupported family, so a missing carrier here is a custody mismatch.
pub(super) fn project_saturating_integer_arithmetic(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (scalar_type, left, right) = match &node.operation {
        AbstractOperation::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
            ..
        } => (*scalar_type, *left, *right),
        _ => unreachable!("dispatched project_saturating_integer_arithmetic"),
    };
    let carrier = scalar_graph_input::saturating_carrier(scalar_type).ok_or(Error::custody())?;
    if [left, right].iter().any(|value| {
        scalar_graph_input::value_type(optimized, *value) != Some(ScalarType::Integer(scalar_type))
    }) {
        return Err(Error::custody());
    }
    Ok(match &node.operation {
        AbstractOperation::SaturatingIntegerAdd { .. } => {
            LegalizedScalarInstructionKind::SaturatingAdd {
                carrier,
                left,
                right,
            }
        }
        AbstractOperation::SaturatingIntegerSubtract { .. } => {
            LegalizedScalarInstructionKind::SaturatingSubtract {
                carrier,
                left,
                right,
            }
        }
        _ => LegalizedScalarInstructionKind::SaturatingMultiply {
            carrier,
            left,
            right,
        },
    })
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
    let carrier = scalar_graph_input::saturating_carrier(*scalar_type).ok_or(Error::custody())?;
    if [left, right].iter().any(|value| {
        scalar_graph_input::value_type(optimized, **value)
            != Some(ScalarType::Integer(*scalar_type))
    }) {
        return Err(Error::custody());
    }
    // Saturating clamps signed MIN / -1 to MAX and unsigned division never
    // overflows; neither defines division by zero, so the accepted
    // nonzero-divisor fact stays with the instruction for every carrier.
    let accepted_fact =
        accepted_nonzero_divisor_fact(optimized, unit, *psi_operation, *obligation)?;
    Ok(LegalizedScalarInstructionKind::SaturatingDivide {
        carrier,
        left: *left,
        right: *right,
        obligation: *obligation,
        accepted_fact,
    })
}

/// Saturating remainder names the carrier the source operation declares.
/// The mathematical remainder always lies inside the carrier, so saturation
/// adds no clamp; the accepted nonzero-divisor fact stays with the
/// instruction because saturating does not define a zero divisor.
pub(super) fn project_saturating_integer_remainder(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::SaturatingIntegerRemainder {
        psi_operation,
        obligation,
        scalar_type,
        left,
        right,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_saturating_integer_remainder")
    };
    let carrier = scalar_graph_input::saturating_carrier(*scalar_type).ok_or(Error::custody())?;
    if [left, right].iter().any(|value| {
        scalar_graph_input::value_type(optimized, **value)
            != Some(ScalarType::Integer(*scalar_type))
    }) {
        return Err(Error::custody());
    }
    let accepted_fact =
        accepted_nonzero_divisor_fact(optimized, unit, *psi_operation, *obligation)?;
    Ok(LegalizedScalarInstructionKind::SaturatingRemainder {
        carrier,
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
    let fact = facts.next().ok_or(Error::custody())?;
    if facts.next().is_some()
        || !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if *referenced == obligation && *support == psi_operation))
    {
        return Err(Error::custody());
    }
    Ok(fact.identity)
}

/// Wrapping remainder admits every fixed native carrier. Wrapping defines
/// MIN % -1 as zero, not a failed Exact quotient, but it does not define
/// division by zero: the accepted nonzero-divisor fact stays with the
/// instruction.
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
    wrapping_division_operands(optimized, *scalar_type, *left, *right)?;
    Ok(LegalizedScalarInstructionKind::WrappingRemainder {
        left: *left,
        right: *right,
        obligation: *obligation,
        accepted_fact: accepted_nonzero_divisor_fact(optimized, unit, *psi_operation, *obligation)?,
    })
}

/// Wrapping division admits every fixed native carrier: a signed MIN / -1
/// quotient wraps back to MIN (selection normalizes the widened narrow
/// quotient and guards the i64 one), and unsigned division never overflows.
/// Division by zero stays the accepted obligation carried by the
/// instruction.
pub(super) fn project_wrapping_integer_divide(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::WrappingIntegerDivide {
        psi_operation,
        obligation,
        scalar_type,
        left,
        right,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_wrapping_integer_divide")
    };
    wrapping_division_operands(optimized, *scalar_type, *left, *right)?;
    Ok(LegalizedScalarInstructionKind::WrappingDivide {
        left: *left,
        right: *right,
        obligation: *obligation,
        accepted_fact: accepted_nonzero_divisor_fact(optimized, unit, *psi_operation, *obligation)?,
    })
}

/// Node admission already refused every carrier wrapping division is not
/// realized for, so a carrier or operand type disagreement here is custody.
fn wrapping_division_operands(
    optimized: &optimization_unit::PsiOptimizationFunction,
    scalar_type: semantic_vocabulary::IntegerType,
    left: semantic_vocabulary::ValueId,
    right: semantic_vocabulary::ValueId,
) -> Result<(), LegalizationError> {
    if !scalar_graph_input::supports_wrapping_division(scalar_type)
        || [left, right].iter().any(|value| {
            scalar_graph_input::value_type(optimized, *value)
                != Some(ScalarType::Integer(scalar_type))
        })
    {
        return Err(Error::custody());
    }
    Ok(())
}

/// Shifts keep an independently typed count next to the shifted value. The
/// wrapping forms reduce the count modulo the value width natively; the
/// exact forms retain the accepted in-range fact discharging the shift
/// obligation, matching every other proof-bearing operation.
pub(super) fn project_shift(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (psi_operation, value_type, count_type, value, count) = match &node.operation {
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | AbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            value_type,
            count_type,
            value,
            count,
            ..
        } => (*psi_operation, *value_type, *count_type, *value, *count),
        _ => unreachable!("dispatched project_shift"),
    };
    if scalar_graph_input::scalar_shape(ScalarType::Integer(value_type)).is_none()
        || scalar_graph_input::scalar_shape(ScalarType::Integer(count_type)).is_none()
        || scalar_graph_input::value_type(optimized, value) != Some(ScalarType::Integer(value_type))
        || scalar_graph_input::value_type(optimized, count) != Some(ScalarType::Integer(count_type))
    {
        return Err(Error::custody());
    }
    Ok(match &node.operation {
        AbstractOperation::WrappingIntegerShiftLeft { .. } => {
            LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
        }
        AbstractOperation::WrappingIntegerShiftRight { .. } => {
            LegalizedScalarInstructionKind::WrappingShiftRight { value, count }
        }
        AbstractOperation::ExactIntegerShiftLeft { obligation, .. } => {
            LegalizedScalarInstructionKind::ExactShiftLeft {
                value,
                count,
                obligation: *obligation,
                accepted_fact: {
                    let fact = unit
                        .accepted_obligation_facts
                        .iter()
                        .find(|fact| {
                            fact.machine == optimized.machine
                                && fact.operation == psi_operation
                                && fact.obligation == *obligation
                        })
                        .ok_or(Error::custody())?;
                    if !optimized.facts.iter().any(|fact| matches!(fact,
                            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                            if referenced == obligation && *support == psi_operation)) {
                        return Err(Error::custody());
                    }
                    fact.identity
                },
            }
        }
        AbstractOperation::ExactIntegerShiftRight { obligation, .. } => {
            LegalizedScalarInstructionKind::ExactShiftRight {
                value,
                count,
                obligation: *obligation,
                accepted_fact: {
                    let fact = unit
                        .accepted_obligation_facts
                        .iter()
                        .find(|fact| {
                            fact.machine == optimized.machine
                                && fact.operation == psi_operation
                                && fact.obligation == *obligation
                        })
                        .ok_or(Error::custody())?;
                    if !optimized.facts.iter().any(|fact| matches!(fact,
                            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                            if referenced == obligation && *support == psi_operation)) {
                        return Err(Error::custody());
                    }
                    fact.identity
                },
            }
        }
        _ => unreachable!("dispatched project_shift"),
    })
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
    }
    | AbstractOperation::ExactIntegerRemainder {
        psi_operation,
        obligation,
        left,
        right,
        ..
    }
    | AbstractOperation::ExactIntegerMultiply {
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
            .ok_or(Error::custody())?;
        if !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if referenced == obligation && support == psi_operation)) {
                return Err(Error::custody());
            }
        LegalizedScalarInstructionKind::ExactBinary {
            operator: if matches!(node.operation, AbstractOperation::ExactIntegerAdd { .. }) {
                LegalizedExactIntegerOperator::Add
            } else if matches!(node.operation, AbstractOperation::ExactIntegerDivide { .. }) {
                LegalizedExactIntegerOperator::Divide
            } else if matches!(
                node.operation,
                AbstractOperation::ExactIntegerRemainder { .. }
            ) {
                LegalizedExactIntegerOperator::Remainder
            } else if matches!(
                node.operation,
                AbstractOperation::ExactIntegerMultiply { .. }
            ) {
                LegalizedExactIntegerOperator::Multiply
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
            _ => return Err(Error::custody()),
        };
        let operand_type =
            scalar_graph_input::value_type(optimized, *left).ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            left: *left,
            right: *right,
        }
    };
    Ok(kind)
}
