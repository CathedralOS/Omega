use super::super::common::{parameter_types, parameter_value, scalar_terminal_artifact};
use super::super::*;

pub(crate) fn saturating_integer_multiply_parameters_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    assert!(
        parameter_count >= 2,
        "saturating-multiply fixture needs two operands"
    );
    let scalar_type = ScalarType::Integer(integer_type);
    scalar_terminal_artifact(
        scalar_type,
        parameter_types(scalar_type, parameter_count),
        Some(OperationKind::SaturatingIntegerMultiply {
            left: parameter_value(parameter_count - 2),
            right: parameter_value(parameter_count - 1),
        }),
        None,
        None,
    )
}
