use super::{
    AbstractOperation, AbstractOperationPlan, ObligationId, OperationId, ScalarType, ValueId,
    parameter_return_plan,
};

pub(in crate::tests) fn wrapping_integer_shift_left_parameters_plan(
    parameter_types: &[ScalarType],
    value_parameter: usize,
    count_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, count_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(value_type) = function.parameters[value_parameter].scalar_type else {
        panic!("wrapping-shift-left fixture requires an integer value")
    };
    let ScalarType::Integer(count_type) = function.parameters[count_parameter].scalar_type else {
        panic!("wrapping-shift-left fixture requires an integer count")
    };
    let result = ValueId::new(6_901).unwrap();
    let value = function.parameters[value_parameter].value;
    let count = function.parameters[count_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation: OperationId::new(6_900).unwrap(),
            result,
            value_type,
            count_type,
            value,
            count,
        },
    );
    super::unary::finish_integer_expression(function, result, value_type);
    plan
}

pub(in crate::tests) fn wrapping_integer_shift_right_parameters_plan(
    parameter_types: &[ScalarType],
    value_parameter: usize,
    count_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, count_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(value_type) = function.parameters[value_parameter].scalar_type else {
        panic!("wrapping-shift-right fixture requires an integer value")
    };
    let ScalarType::Integer(count_type) = function.parameters[count_parameter].scalar_type else {
        panic!("wrapping-shift-right fixture requires an integer count")
    };
    let result = ValueId::new(7_101).unwrap();
    let value = function.parameters[value_parameter].value;
    let count = function.parameters[count_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::WrappingIntegerShiftRight {
            psi_operation: OperationId::new(7_100).unwrap(),
            result,
            value_type,
            count_type,
            value,
            count,
        },
    );
    super::unary::finish_integer_expression(function, result, value_type);
    plan
}

pub(in crate::tests) fn exact_integer_shift_right_parameters_plan(
    parameter_types: &[ScalarType],
    value_parameter: usize,
    count_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, count_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(value_type) = function.parameters[value_parameter].scalar_type else {
        panic!("exact-shift-right fixture requires an integer value")
    };
    let ScalarType::Integer(count_type) = function.parameters[count_parameter].scalar_type else {
        panic!("exact-shift-right fixture requires an integer count")
    };
    let result = ValueId::new(7_301).unwrap();
    let value = function.parameters[value_parameter].value;
    let count = function.parameters[count_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::ExactIntegerShiftRight {
            psi_operation: OperationId::new(7_300).unwrap(),
            obligation: ObligationId::new(7_302).unwrap(),
            result,
            value_type,
            count_type,
            value,
            count,
        },
    );
    super::unary::finish_integer_expression(function, result, value_type);
    plan
}

pub(in crate::tests) fn exact_integer_shift_left_parameters_plan(
    parameter_types: &[ScalarType],
    value_parameter: usize,
    count_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, count_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(value_type) = function.parameters[value_parameter].scalar_type else {
        panic!("exact-shift-left fixture requires an integer value")
    };
    let ScalarType::Integer(count_type) = function.parameters[count_parameter].scalar_type else {
        panic!("exact-shift-left fixture requires an integer count")
    };
    let result = ValueId::new(7_201).unwrap();
    let value = function.parameters[value_parameter].value;
    let count = function.parameters[count_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::ExactIntegerShiftLeft {
            psi_operation: OperationId::new(7_200).unwrap(),
            obligation: ObligationId::new(7_202).unwrap(),
            result,
            value_type,
            count_type,
            value,
            count,
        },
    );
    super::unary::finish_integer_expression(function, result, value_type);
    plan
}
