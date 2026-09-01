//! Closed scalar-literal operation admission for the exact rule.

use omega_abstract_operations::AbstractOperation as O;
use psi_core::ScalarType;

use crate::rules::passes::support::DeadScalarShape;

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    match operation {
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            ..
        } => Some(DeadScalarShape {
            source_operation: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
        }),
        O::BooleanConstant {
            psi_operation,
            result,
            ..
        } => Some(DeadScalarShape {
            source_operation: *psi_operation,
            result: *result,
            scalar_type: ScalarType::Boolean,
        }),
        _ => None,
    }
}
