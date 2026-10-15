//! Scalar arithmetic, logic, comparison and cast operations projected to
//! the legalized instruction kind that realizes each.

use super::super::{Error, PsiOptimizationUnit};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::{
    AbstractOperation, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
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
                    && fact.owner == optimization_unit::PsiProvenance::Operation(*psi_operation)
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

/// One integer arithmetic node's fields, read once by the dispatch.
#[derive(Clone, Copy)]
pub(super) struct IntegerArithmeticNode {
    pub(super) psi_operation: semantic_vocabulary::OperationId,
    pub(super) scalar_type: semantic_vocabulary::IntegerType,
    pub(super) arithmetic: IntegerArithmetic,
    pub(super) left: semantic_vocabulary::ValueId,
    pub(super) right: semantic_vocabulary::ValueId,
}

/// Project one integer arithmetic pair to the legalized instruction kind that
/// realizes it. Wrapping add, subtract and multiply need no carrier or
/// obligation beyond node admission; every other pair checks its carrier and,
/// when it carries one, its accepted obligation.
pub(super) fn project_integer_arithmetic(
    node: IntegerArithmeticNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    let IntegerArithmeticNode { left, right, .. } = node;
    match (node.arithmetic.policy(), node.arithmetic.operator()) {
        (Policy::Wrapping, Operator::Add) => {
            Ok(LegalizedScalarInstructionKind::WrappingAdd { left, right })
        }
        (Policy::Wrapping, Operator::Subtract) => {
            Ok(LegalizedScalarInstructionKind::WrappingSubtract { left, right })
        }
        (Policy::Wrapping, Operator::Multiply) => {
            Ok(LegalizedScalarInstructionKind::WrappingMultiply { left, right })
        }
        (Policy::Wrapping, Operator::Divide | Operator::Remainder) => {
            project_wrapping_integer_division(node, optimized, unit)
        }
        (Policy::Saturating, Operator::Add | Operator::Subtract | Operator::Multiply) => {
            project_saturating_integer_arithmetic(node, optimized)
        }
        (Policy::Saturating, Operator::Divide | Operator::Remainder) => {
            project_saturating_integer_division(node, optimized, unit)
        }
        (Policy::Exact, _) => project_exact_integer_arithmetic(node, optimized, unit),
    }
}

/// Saturating add, subtract, and multiply name the carrier the source
/// operation declares and carry no obligation: saturation defines every
/// result. Node admission already rejected every non-carrier width as an
/// unsupported family, so a missing carrier here is a custody mismatch.
fn project_saturating_integer_arithmetic(
    node: IntegerArithmeticNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let IntegerArithmeticNode {
        scalar_type,
        arithmetic,
        left,
        right,
        ..
    } = node;
    let carrier = scalar_graph_input::saturating_carrier(scalar_type).ok_or(Error::custody())?;
    if [left, right].iter().any(|value| {
        scalar_graph_input::value_type(optimized, *value) != Some(ScalarType::Integer(scalar_type))
    }) {
        return Err(Error::custody());
    }
    Ok(match arithmetic.operator() {
        IntegerArithmeticOperator::Add => LegalizedScalarInstructionKind::SaturatingAdd {
            carrier,
            left,
            right,
        },
        IntegerArithmeticOperator::Subtract => LegalizedScalarInstructionKind::SaturatingSubtract {
            carrier,
            left,
            right,
        },
        _ => LegalizedScalarInstructionKind::SaturatingMultiply {
            carrier,
            left,
            right,
        },
    })
}

/// A Trapping operation names the form node admission accepted and carries
/// no obligation: the realized check is the policy. A conversion also keeps
/// its operand's exact declared carrier beside the form's source sign.
pub(super) fn project_trapping_integer(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let AbstractOperation::TrappingInteger {
        scalar_type,
        operand_type,
        operation,
        ..
    } = &node.operation
    else {
        unreachable!("dispatched project_trapping_integer");
    };
    let form =
        scalar_graph_input::trapping_form(*scalar_type, *operand_type, operation.primitive())
            .ok_or(Error::custody())?;
    if !scalar_graph_input::trapping_operands_rejoin(
        optimized,
        *scalar_type,
        *operand_type,
        *operation,
    ) {
        return Err(Error::custody());
    }
    Ok(match *operation {
        terminal_psi::TrappingIntegerOperation::Convert { operand } => {
            LegalizedScalarInstructionKind::TrappingConvert {
                form,
                source: scalar_graph_input::saturating_carrier(*operand_type)
                    .ok_or(Error::custody())?,
                operand,
            }
        }
        binary => {
            let [left, right] = binary.operands()[..] else {
                return Err(Error::custody());
            };
            LegalizedScalarInstructionKind::TrappingBinary { form, left, right }
        }
    })
}

/// Saturating division and remainder name the carrier the source operation
/// declares. Saturating clamps signed MIN / -1 to MAX, unsigned division
/// never overflows, and a mathematical remainder always lies inside the
/// carrier; none defines division by zero, so the accepted nonzero-divisor
/// fact stays with the instruction for every carrier.
fn project_saturating_integer_division(
    node: IntegerArithmeticNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let IntegerArithmeticNode {
        psi_operation,
        scalar_type,
        arithmetic,
        left,
        right,
    } = node;
    let obligation = arithmetic.obligation().ok_or(Error::custody())?;
    let carrier = scalar_graph_input::saturating_carrier(scalar_type).ok_or(Error::custody())?;
    if [left, right].iter().any(|value| {
        scalar_graph_input::value_type(optimized, *value) != Some(ScalarType::Integer(scalar_type))
    }) {
        return Err(Error::custody());
    }
    let accepted_fact = accepted_nonzero_divisor_fact(optimized, unit, psi_operation, obligation)?;
    Ok(
        if arithmetic.operator() == IntegerArithmeticOperator::Divide {
            LegalizedScalarInstructionKind::SaturatingDivide {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            }
        } else {
            LegalizedScalarInstructionKind::SaturatingRemainder {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            }
        },
    )
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
            && fact.owner == optimization_unit::PsiProvenance::Operation(psi_operation)
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

/// Wrapping division and remainder admit every fixed native carrier. A
/// signed MIN / -1 quotient wraps back to MIN (selection normalizes the
/// widened narrow quotient and guards the i64 one), wrapping defines
/// MIN % -1 as zero, not a failed Exact quotient, and unsigned division never
/// overflows. Division by zero stays the accepted obligation carried by the
/// instruction.
fn project_wrapping_integer_division(
    node: IntegerArithmeticNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let IntegerArithmeticNode {
        psi_operation,
        scalar_type,
        arithmetic,
        left,
        right,
    } = node;
    let obligation = arithmetic.obligation().ok_or(Error::custody())?;
    wrapping_division_operands(optimized, scalar_type, left, right)?;
    let accepted_fact = accepted_nonzero_divisor_fact(optimized, unit, psi_operation, obligation)?;
    Ok(
        if arithmetic.operator() == IntegerArithmeticOperator::Divide {
            LegalizedScalarInstructionKind::WrappingDivide {
                left,
                right,
                obligation,
                accepted_fact,
            }
        } else {
            LegalizedScalarInstructionKind::WrappingRemainder {
                left,
                right,
                obligation,
                accepted_fact,
            }
        },
    )
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
    let AbstractOperation::IntegerShift {
        psi_operation,
        value_type,
        count_type,
        policy,
        direction,
        value,
        count,
        ..
    } = node.operation
    else {
        unreachable!("dispatched project_shift")
    };
    if scalar_graph_input::scalar_shape(ScalarType::Integer(value_type)).is_none()
        || scalar_graph_input::scalar_shape(ScalarType::Integer(count_type)).is_none()
        || scalar_graph_input::value_type(optimized, value) != Some(ScalarType::Integer(value_type))
        || scalar_graph_input::value_type(optimized, count) != Some(ScalarType::Integer(count_type))
    {
        return Err(Error::custody());
    }
    let IntegerShiftPolicy::Exact { obligation } = policy else {
        return Ok(match direction {
            IntegerShiftDirection::Left => {
                LegalizedScalarInstructionKind::WrappingShiftLeft { value, count }
            }
            IntegerShiftDirection::Right => {
                LegalizedScalarInstructionKind::WrappingShiftRight { value, count }
            }
        });
    };
    let accepted_fact = {
        let fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| {
                fact.machine == optimized.machine
                    && fact.owner == optimization_unit::PsiProvenance::Operation(psi_operation)
                    && fact.obligation == obligation
            })
            .ok_or(Error::custody())?;
        if !optimized.facts.iter().any(|fact| matches!(fact,
                optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                if *referenced == obligation && *support == psi_operation)) {
            return Err(Error::custody());
        }
        fact.identity
    };
    Ok(match direction {
        IntegerShiftDirection::Left => LegalizedScalarInstructionKind::ExactShiftLeft {
            value,
            count,
            obligation,
            accepted_fact,
        },
        IntegerShiftDirection::Right => LegalizedScalarInstructionKind::ExactShiftRight {
            value,
            count,
            obligation,
            accepted_fact,
        },
    })
}

fn project_exact_integer_arithmetic(
    node: IntegerArithmeticNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let IntegerArithmeticNode {
        psi_operation,
        arithmetic,
        left,
        right,
        ..
    } = node;
    let obligation = arithmetic.obligation().ok_or(Error::custody())?;
    let fact = unit
        .accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == optimized.machine
                && fact.owner == optimization_unit::PsiProvenance::Operation(psi_operation)
                && fact.obligation == obligation
        })
        .ok_or(Error::custody())?;
    if !optimized.facts.iter().any(|fact| matches!(fact,
            optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
            if *referenced == obligation && *support == psi_operation)) {
        return Err(Error::custody());
    }
    Ok(LegalizedScalarInstructionKind::ExactBinary {
        operator: match arithmetic.operator() {
            IntegerArithmeticOperator::Add => LegalizedExactIntegerOperator::Add,
            IntegerArithmeticOperator::Subtract => LegalizedExactIntegerOperator::Subtract,
            IntegerArithmeticOperator::Multiply => LegalizedExactIntegerOperator::Multiply,
            IntegerArithmeticOperator::Divide => LegalizedExactIntegerOperator::Divide,
            IntegerArithmeticOperator::Remainder => LegalizedExactIntegerOperator::Remainder,
        },
        left,
        right,
        obligation,
        accepted_fact: fact.identity,
    })
}

pub(super) fn project_boolean_equal(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
    let (AbstractOperation::BooleanEqual { left, right, .. }
    | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
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
        let right_operand_type =
            scalar_graph_input::value_type(optimized, *right).ok_or(Error::custody())?;
        LegalizedScalarInstructionKind::Compare {
            predicate,
            operand_type,
            right_operand_type,
            left: *left,
            right: *right,
        }
    };
    Ok(kind)
}
