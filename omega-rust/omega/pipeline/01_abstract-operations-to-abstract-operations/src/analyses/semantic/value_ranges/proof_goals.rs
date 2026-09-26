//! Exact abstract operations whose accepted obligations imply integer ranges.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{ObligationId, OperationId, ScalarTerm, ScalarType};
use terminal_semantics::CanonicalScalarGoal;

pub(super) fn for_operation(
    operation: &O,
) -> Option<(OperationId, ObligationId, CanonicalScalarGoal)> {
    let value_term = |id, scalar_type| ScalarTerm::value(id, ScalarType::Integer(scalar_type));
    match operation {
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            value_type,
            count_type,
            count,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactShiftCount {
                value_type: *value_type,
                count_type: *count_type,
                count: value_term(*count, *count_type),
            },
        )),
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            left,
            right,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            left,
            right,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type: *scalar_type,
                left: value_term(*left, *scalar_type),
                right: value_term(*right, *scalar_type),
            },
        )),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            scalar_type,
            right,
            ..
        } => Some((
            *psi_operation,
            *obligation,
            CanonicalScalarGoal::NonzeroDivisor {
                integer_type: *scalar_type,
                divisor: value_term(*right, *scalar_type),
            },
        )),
        _ => None,
    }
}
