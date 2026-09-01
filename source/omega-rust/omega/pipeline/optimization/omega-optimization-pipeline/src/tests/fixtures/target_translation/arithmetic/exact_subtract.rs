use super::super::common::{parameter_types, parameter_value, scalar_terminal_artifact};
use super::super::*;

pub(crate) fn exact_integer_subtract_parameters_return_artifact(
    integer_type: IntegerType,
    parameter_count: usize,
) -> (Vec<u8>, Vec<u8>) {
    assert!(
        parameter_count >= 2,
        "exact-subtract fixture needs two operands"
    );
    let scalar_type = ScalarType::Integer(integer_type);
    let obligation = ObligationId::new(30_009).unwrap();
    scalar_terminal_artifact(
        scalar_type,
        parameter_types(scalar_type, parameter_count),
        Some(OperationKind::ExactIntegerSubtract {
            left: parameter_value(parameter_count - 2),
            right: parameter_value(parameter_count - 1),
            obligation,
        }),
        None,
        Some(obligation),
    )
}
