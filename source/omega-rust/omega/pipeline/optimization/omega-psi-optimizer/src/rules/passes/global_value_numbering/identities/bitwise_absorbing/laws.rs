//! Closed integer bitwise absorbing-literal partition.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::TotalScalarIdentityKind;
use psi_core::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};

use super::super::TotalScalarIdentityShape;

/// Return the four exact-width absorbing laws in canonical operation and
/// left-literal/right-literal order. The law operand is also the replacement.
pub(super) fn classify(
    operation: &O,
) -> Vec<TotalScalarIdentityShape> {
    match operation {
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
            zero(*scalar_type),
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => pair(
            *psi_operation,
            *result,
            *scalar_type,
            *left,
            *right,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
            all_ones(*scalar_type),
        ),
        _ => Vec::new(),
    }
}

fn pair(
    source_operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
    left_identity: TotalScalarIdentityKind,
    right_identity: TotalScalarIdentityKind,
    law_value: IntegerValue,
) -> Vec<TotalScalarIdentityShape> {
    vec![
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: left,
            law_operand: left,
            scalar_type,
            law_operand_type: scalar_type,
            identity: left_identity,
            expected_law_value: law_value,
        },
        TotalScalarIdentityShape {
            source_operation,
            result,
            replacement: right,
            law_operand: right,
            scalar_type,
            law_operand_type: scalar_type,
            identity: right_identity,
            expected_law_value: law_value,
        },
    ]
}

fn all_ones(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(-1),
        IntegerSign::Unsigned => scalar_type.maximum_value(),
    }
}

const fn zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}
