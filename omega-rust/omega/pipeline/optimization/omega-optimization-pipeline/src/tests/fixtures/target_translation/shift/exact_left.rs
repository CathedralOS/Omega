use super::super::common::{parameter_value, scalar_terminal_artifact};
use super::super::*;

pub(crate) fn exact_integer_shift_left_parameters_return_artifact(
    value_type: IntegerType,
    count_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    assert!(parameter_count >= 2, "shift fixture needs two operands");
    let result_type = ScalarType::Integer(value_type);
    let mut parameter_types = vec![result_type; parameter_count];
    parameter_types[parameter_count - 1] = ScalarType::Integer(count_type);
    let obligation = ObligationId::new(30_010).unwrap();
    scalar_terminal_artifact(
        result_type,
        parameter_types,
        Some(OperationKind::ExactIntegerShiftLeft {
            value: parameter_value(parameter_count - 2),
            count: parameter_value(parameter_count - 1),
            obligation,
        }),
        None,
        Some(obligation),
    )
}
