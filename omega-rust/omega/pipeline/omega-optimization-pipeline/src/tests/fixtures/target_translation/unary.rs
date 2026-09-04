use super::common::{parameter_types, parameter_value, scalar_terminal_artifact};
use super::*;

pub(crate) fn boolean_not_parameter_return_artifact(parameter_count: usize) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Boolean,
        parameter_types(ScalarType::Boolean, parameter_count),
        Some(OperationKind::BooleanNot {
            operand: parameter_value(parameter_count - 1),
        }),
        None,
        None,
    )
}

pub(crate) fn integer_bitwise_not_parameter_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    let scalar_type = ScalarType::Integer(integer_type);
    scalar_terminal_artifact(
        scalar_type,
        parameter_types(scalar_type, parameter_count),
        Some(OperationKind::IntegerBitwiseNot {
            operand: parameter_value(parameter_count - 1),
        }),
        None,
        None,
    )
}

pub(crate) fn integer_widen_parameter_return_artifact(
    source_type: IntegerType,
    target_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Integer(target_type),
        parameter_types(ScalarType::Integer(source_type), parameter_count),
        Some(OperationKind::IntegerWiden {
            operand: parameter_value(parameter_count - 1),
        }),
        None,
        None,
    )
}

pub(crate) fn integer_exact_cast_parameter_return_artifact(
    source_type: IntegerType,
    target_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    let obligation = ObligationId::new(30_009).unwrap();
    scalar_terminal_artifact(
        ScalarType::Integer(target_type),
        parameter_types(ScalarType::Integer(source_type), parameter_count),
        Some(OperationKind::IntegerExactCast {
            operand: parameter_value(parameter_count - 1),
            obligation,
        }),
        None,
        Some(obligation),
    )
}
