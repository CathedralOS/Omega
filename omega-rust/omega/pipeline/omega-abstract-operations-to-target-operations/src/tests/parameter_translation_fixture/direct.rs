use super::*;

pub(in crate::tests) fn uniform_integer_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    parameter_return_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 1,
    )
}

pub(in crate::tests) fn uniform_boolean_plan(parameter_count: usize) -> AbstractOperationPlan {
    parameter_return_plan(
        &vec![ScalarType::Boolean; parameter_count],
        parameter_count - 1,
    )
}
