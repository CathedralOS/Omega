//! Closed wrapping multiplication annihilation partition.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;
use psi_core::{IntegerSign, IntegerType, IntegerValue};

use super::super::TotalScalarIdentityShape;

/// Return left-zero then right-zero so ties remain deterministic.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    let O::WrappingIntegerMultiply {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    } = operation
    else {
        return Vec::new();
    };
    vec![
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *left,
            law_operand: *left,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft,
            expected_law_value: integer_zero(*scalar_type),
        },
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *right,
            law_operand: *right,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight,
            expected_law_value: integer_zero(*scalar_type),
        },
    ]
}

const fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}
