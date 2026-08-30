use super::*;

pub(in crate::tests) fn boolean_equal_parameters_plan(
    parameter_types: &[ScalarType],
    left: usize,
    right: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, left);
    let function = &mut plan.functions[0];
    let result = ValueId::new(3_801).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::BooleanEqual {
            psi_operation: OperationId::new(3_800).unwrap(),
            result,
            left: function.parameters[left].value,
            right: function.parameters[right].value,
        },
    );
    finish_comparison(function, result);
    plan
}

pub(in crate::tests) fn uniform_boolean_equal_plan(
    parameter_count: usize,
) -> AbstractOperationPlan {
    boolean_equal_parameters_plan(
        &vec![ScalarType::Boolean; parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

pub(in crate::tests) fn integer_equal_parameters_plan(
    parameter_types: &[ScalarType],
    left: usize,
    right: usize,
) -> AbstractOperationPlan {
    integer_comparison_plan(
        parameter_types,
        left,
        right,
        3_900,
        3_901,
        |psi_operation, result, left, right| AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        },
    )
}

pub(in crate::tests) fn uniform_integer_equal_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_equal_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

pub(in crate::tests) fn integer_less_than_parameters_plan(
    parameter_types: &[ScalarType],
    left: usize,
    right: usize,
) -> AbstractOperationPlan {
    integer_comparison_plan(
        parameter_types,
        left,
        right,
        4_000,
        4_001,
        |psi_operation, result, left, right| AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        },
    )
}

pub(in crate::tests) fn uniform_integer_less_than_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_less_than_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

pub(in crate::tests) fn integer_less_or_equal_parameters_plan(
    parameter_types: &[ScalarType],
    left: usize,
    right: usize,
) -> AbstractOperationPlan {
    integer_comparison_plan(
        parameter_types,
        left,
        right,
        4_100,
        4_101,
        |psi_operation, result, left, right| AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        },
    )
}

pub(in crate::tests) fn uniform_integer_less_or_equal_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_less_or_equal_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

fn integer_comparison_plan(
    parameter_types: &[ScalarType],
    left: usize,
    right: usize,
    operation: u64,
    value: u64,
    build: impl FnOnce(OperationId, ValueId, ValueId, ValueId) -> AbstractOperation,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, left);
    let function = &mut plan.functions[0];
    let result = ValueId::new(value).unwrap();
    function.operations.insert(
        0,
        build(
            OperationId::new(operation).unwrap(),
            result,
            function.parameters[left].value,
            function.parameters[right].value,
        ),
    );
    finish_comparison(function, result);
    plan
}

fn finish_comparison(function: &mut AbstractFunction, value: ValueId) {
    let AbstractOperation::Return {
        value: returned,
        scalar_type,
        ..
    } = &mut function.operations[1]
    else {
        unreachable!()
    };
    *returned = value;
    *scalar_type = ScalarType::Boolean;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
}
