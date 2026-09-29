//! Exact abstract operations whose accepted obligations imply integer ranges.

use abstract_operations::{
    AbstractOperation as O, IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{ObligationId, OperationId, ScalarTerm, ScalarType};
use terminal_semantics::CanonicalScalarGoal;

pub(super) fn for_operation(
    operation: &O,
) -> Option<(OperationId, ObligationId, CanonicalScalarGoal)> {
    let value_term = |id, scalar_type| ScalarTerm::value(id, ScalarType::Integer(scalar_type));
    match operation {
        O::IntegerShift {
            psi_operation,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { obligation },
            direction: IntegerShiftDirection::Right,
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
        O::IntegerArithmetic {
            psi_operation,
            scalar_type,
            arithmetic,
            left,
            right,
            ..
        } if arithmetic.operator().is_division() => {
            let obligation = arithmetic.obligation()?;
            // An exact quotient also excludes the signed `MIN / -1` overflow;
            // wrapping and saturating division define that case and only
            // need a nonzero divisor.
            let goal = match arithmetic.policy() {
                IntegerOverflowPolicy::Exact => CanonicalScalarGoal::ExactDivisionDefined {
                    integer_type: *scalar_type,
                    left: value_term(*left, *scalar_type),
                    right: value_term(*right, *scalar_type),
                },
                IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating => {
                    CanonicalScalarGoal::NonzeroDivisor {
                        integer_type: *scalar_type,
                        divisor: value_term(*right, *scalar_type),
                    }
                }
            };
            Some((*psi_operation, obligation, goal))
        }
        _ => None,
    }
}
