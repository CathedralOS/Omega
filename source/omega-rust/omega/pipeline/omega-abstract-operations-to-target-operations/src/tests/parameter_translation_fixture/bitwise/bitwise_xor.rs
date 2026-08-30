use super::super::*;

pub(in crate::tests) fn integer_bitwise_xor_parameters_plan(
    parameter_types: &[ScalarType],
    left_parameter: usize,
    right_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, right_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(scalar_type) = function.parameters[left_parameter].scalar_type else {
        panic!("integer bitwise-XOR fixture requires integer operands")
    };
    let result = ValueId::new(4_701).unwrap();
    let left = function.parameters[left_parameter].value;
    let right = function.parameters[right_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::IntegerBitwiseXor {
            psi_operation: OperationId::new(4_700).unwrap(),
            result,
            scalar_type,
            left,
            right,
        },
    );
    super::super::unary::finish_integer_expression(function, result, scalar_type);
    plan
}

pub(in crate::tests) fn uniform_integer_bitwise_xor_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_bitwise_xor_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}
