//! Closed wrapping neutral-arithmetic partition.

use abstract_operations::AbstractOperation as O;
use optimization_unit::TotalScalarIdentityKind;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use super::super::TotalScalarIdentityShape;

/// Return the exact laws in canonical left-literal/right-literal tie order.
pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                law_operand: *left,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
                expected_law_value: integer_value(*scalar_type, 0),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
                expected_law_value: integer_value(*scalar_type, 0),
            },
        ],
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *left,
            law_operand: *right,
            scalar_type: *scalar_type,
            law_operand_type: *scalar_type,
            identity: TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
            expected_law_value: integer_value(*scalar_type, 0),
        }],
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => vec![
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *right,
                law_operand: *left,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
                expected_law_value: integer_value(*scalar_type, 1),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
                expected_law_value: integer_value(*scalar_type, 1),
            },
        ],
        _ => Vec::new(),
    }
}

const fn integer_value(scalar_type: IntegerType, value: u128) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value as i128),
        IntegerSign::Unsigned => IntegerValue::Unsigned(value),
    }
}
