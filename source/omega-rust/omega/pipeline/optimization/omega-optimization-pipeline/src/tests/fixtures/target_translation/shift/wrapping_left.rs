use super::super::common::{parameter_value, scalar_terminal_artifact};
use super::super::*;

pub(crate) fn wrapping_integer_shift_left_parameters_return_artifact(
    value_type: IntegerType,
    count_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    assert!(parameter_count >= 2, "shift fixture needs two operands");
    let result_type = ScalarType::Integer(value_type);
    let mut parameter_types = vec![result_type; parameter_count];
    parameter_types[parameter_count - 1] = ScalarType::Integer(count_type);
    scalar_terminal_artifact(
        result_type,
        parameter_types,
        Some(OperationKind::WrappingIntegerShiftLeft {
            value: parameter_value(parameter_count - 2),
            count: parameter_value(parameter_count - 1),
        }),
        None,
        None,
    )
}
