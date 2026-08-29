//! Closed saturating neutral-arithmetic partition.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;
use psi_core::{IntegerSign, IntegerType, IntegerValue};

use super::TotalScalarIdentityShape;

/// Return the five saturating neutral laws in canonical tie order.
///
/// For commutative operations the left-identity row precedes the right row.
/// Signed one-bit integers have no positive-one literal, so the shared literal
/// fact lookup naturally declines both multiplication rows for that type.
pub(in crate::rules::passes) fn saturating_neutral_identity_shapes(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::SaturatingIntegerAdd {
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
                identity: TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
                expected_law_value: integer_value(*scalar_type, 0),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::SaturatingIntegerAddZeroRight,
                expected_law_value: integer_value(*scalar_type, 0),
            },
        ],
        O::SaturatingIntegerSubtract {
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
            identity: TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
            expected_law_value: integer_value(*scalar_type, 0),
        }],
        O::SaturatingIntegerMultiply {
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
                identity: TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
                expected_law_value: integer_value(*scalar_type, 1),
            },
            TotalScalarIdentityShape {
                source_operation: *psi_operation,
                result: *result,
                replacement: *left,
                law_operand: *right,
                scalar_type: *scalar_type,
                law_operand_type: *scalar_type,
                identity: TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
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
