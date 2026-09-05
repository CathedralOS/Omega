//! Closed saturating multiplication annihilation partition.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use super::super::TotalScalarIdentityShape;

/// Return the two saturating multiplication annihilation laws in canonical
/// left-zero/right-zero order. The zero operand is also the replacement.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    let O::SaturatingIntegerMultiply {
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
            identity: TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
            expected_law_value: integer_zero(*scalar_type),
        },
        TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *right,
            law_operand: *right,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
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
