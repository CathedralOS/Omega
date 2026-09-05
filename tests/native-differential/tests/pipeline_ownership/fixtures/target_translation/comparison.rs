use super::common::{parameter_types, parameter_value, scalar_terminal_artifact};
use super::*;

pub(crate) fn boolean_equal_parameters_return_artifact(
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    comparison_artifact(ScalarType::Boolean, parameter_count, |left, right| {
        OperationKind::BooleanEqual { left, right }
    })
}

pub(crate) fn integer_equal_parameters_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    integer_comparison_artifact(integer_type, parameter_count, |left, right| {
        OperationKind::IntegerEqual { left, right }
    })
}

pub(crate) fn integer_less_than_parameters_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    integer_comparison_artifact(integer_type, parameter_count, |left, right| {
        OperationKind::IntegerLessThan { left, right }
    })
}

pub(crate) fn integer_less_or_equal_parameters_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    integer_comparison_artifact(integer_type, parameter_count, |left, right| {
        OperationKind::IntegerLessOrEqual { left, right }
    })
}

fn integer_comparison_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
    operation: impl FnOnce(ValueId, ValueId) -> OperationKind,
) -> (Vec<u8>, Vec<u8>) {
    comparison_artifact(
        ScalarType::Integer(integer_type),
        parameter_count,
        operation,
    )
}

fn comparison_artifact(
    parameter_type: ScalarType,
    parameter_count: usize,
    operation: impl FnOnce(ValueId, ValueId) -> OperationKind,
) -> (Vec<u8>, Vec<u8>) {
    assert!(
        parameter_count >= 2,
        "comparison fixture needs two operands"
    );
    scalar_terminal_artifact(
        ScalarType::Boolean,
        parameter_types(parameter_type, parameter_count),
        Some(operation(
            parameter_value(parameter_count - 2),
            parameter_value(parameter_count - 1),
        )),
        None,
        None,
    )
}
