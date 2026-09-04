use super::super::{
    AbstractOperation, AbstractOperationPlan, IntegerType, ObligationId, OperationId, ScalarType,
    ValueId, parameter_return_plan,
};

pub(in crate::tests) fn wrapping_integer_remainder_parameters_plan(
    parameter_types: &[ScalarType],
    left_parameter: usize,
    right_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, right_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(scalar_type) = function.parameters[left_parameter].scalar_type else {
        panic!("wrapping-integer-remainder fixture requires integer operands")
    };
    let result = ValueId::new(6_301).unwrap();
    let left = function.parameters[left_parameter].value;
    let right = function.parameters[right_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation: OperationId::new(6_300).unwrap(),
            obligation: ObligationId::new(6_302).unwrap(),
            result,
            scalar_type,
            left,
            right,
        },
    );
    super::super::unary::finish_integer_expression(function, result, scalar_type);
    plan
}

pub(in crate::tests) fn uniform_wrapping_integer_remainder_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    wrapping_integer_remainder_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}
