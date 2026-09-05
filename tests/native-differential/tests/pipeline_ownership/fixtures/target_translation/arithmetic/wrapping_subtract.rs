use super::super::common::{parameter_types, parameter_value, scalar_terminal_artifact};
use super::super::*;

pub(crate) fn wrapping_integer_subtract_parameters_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    assert!(
        parameter_count >= 2,
        "wrapping-subtract fixture needs two operands"
    );
    let scalar_type = ScalarType::Integer(integer_type);
    scalar_terminal_artifact(
        scalar_type,
        parameter_types(scalar_type, parameter_count),
        Some(OperationKind::WrappingIntegerSubtract {
            left: parameter_value(parameter_count - 2),
            right: parameter_value(parameter_count - 1),
        }),
        None,
        None,
    )
}
