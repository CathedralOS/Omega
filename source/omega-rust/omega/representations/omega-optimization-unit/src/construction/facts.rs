use super::*;

pub(super) fn collect_fact(operation: &AbstractOperation, facts: &mut Vec<OptimizationFact>) {
    if let Some((obligation, support)) = operation_obligation(operation) {
        facts.push(OptimizationFact::OperationObligationReference {
            obligation,
            support,
        });
    }
    match operation {
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => facts.push(OptimizationFact::BooleanConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            value,
            ..
        } => facts.push(OptimizationFact::IntegerConstant {
            value: *result,
            constant: *value,
            support: *psi_operation,
        }),
        _ => {}
    }
}

fn operation_obligation(operation: &AbstractOperation) -> Option<(ObligationId, OperationId)> {
    use AbstractOperation as O;
    match operation {
        O::IntegerExactCast {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerAdd {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerDivide {
            psi_operation,
            obligation,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            ..
        }
        | O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            ..
        } => Some((*obligation, *psi_operation)),
        _ => None,
    }
}
