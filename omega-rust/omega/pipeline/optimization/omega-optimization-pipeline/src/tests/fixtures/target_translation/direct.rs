use super::common::{parameter_types, scalar_terminal_artifact};
use super::*;

pub(crate) fn integer_parameter_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    let scalar_type = ScalarType::Integer(integer_type);
    scalar_terminal_artifact(
        scalar_type,
        parameter_types(scalar_type, parameter_count),
        None,
        None,
        None,
    )
}

pub(crate) fn boolean_parameter_return_artifact(parameter_count: usize) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(
        ScalarType::Boolean,
        parameter_types(ScalarType::Boolean, parameter_count),
        None,
        None,
        None,
    )
}
