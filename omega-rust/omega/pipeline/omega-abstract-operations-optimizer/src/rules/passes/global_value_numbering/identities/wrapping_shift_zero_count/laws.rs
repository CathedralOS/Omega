//! Closed wrapping shift-zero-count partition.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;
use psi_core::{IntegerSign, IntegerType, IntegerValue};

use super::super::TotalScalarIdentityShape;

pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            law_operand: *count,
            scalar_type: *value_type,
            law_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
            expected_law_value: integer_zero(*count_type),
        }],
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            law_operand: *count,
            scalar_type: *value_type,
            law_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
            expected_law_value: integer_zero(*count_type),
        }],
        _ => Vec::new(),
    }
}

const fn integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}
