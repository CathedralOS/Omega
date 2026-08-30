use super::*;

pub(in crate::tests) fn boolean_not_parameter_plan(
    parameter_types: &[ScalarType],
    operand_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, operand_parameter);
    let function = &mut plan.functions[0];
    let result = ValueId::new(3_701).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::BooleanNot {
            psi_operation: OperationId::new(3_700).unwrap(),
            result,
            operand,
        },
    );
    finish_boolean_expression(function, result);
    plan
}

pub(in crate::tests) fn uniform_boolean_not_plan(parameter_count: usize) -> AbstractOperationPlan {
    boolean_not_parameter_plan(
        &vec![ScalarType::Boolean; parameter_count],
        parameter_count - 1,
    )
}

pub(in crate::tests) fn integer_bitwise_not_parameter_plan(
    parameter_types: &[ScalarType],
    operand_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, operand_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(scalar_type) = function.parameters[operand_parameter].scalar_type
    else {
        panic!("integer bitwise-not fixture requires an integer operand")
    };
    let result = ValueId::new(4_201).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::IntegerBitwiseNot {
            psi_operation: OperationId::new(4_200).unwrap(),
            result,
            scalar_type,
            operand,
        },
    );
    finish_integer_expression(function, result, scalar_type);
    plan
}

pub(in crate::tests) fn uniform_integer_bitwise_not_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_bitwise_not_parameter_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 1,
    )
}

pub(in crate::tests) fn integer_widen_parameter_plan(
    parameter_types: &[ScalarType],
    operand_parameter: usize,
    target_type: IntegerType,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, operand_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(source_type) = function.parameters[operand_parameter].scalar_type
    else {
        panic!("integer widen fixture requires an integer operand")
    };
    let result = ValueId::new(4_301).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::IntegerWiden {
            psi_operation: OperationId::new(4_300).unwrap(),
            result,
            source_type,
            target_type,
            operand,
        },
    );
    finish_integer_expression(function, result, target_type);
    plan
}

pub(in crate::tests) fn uniform_integer_widen_plan(
    source_type: IntegerType,
    target_type: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_widen_parameter_plan(
        &vec![ScalarType::Integer(source_type); parameter_count],
        parameter_count - 1,
        target_type,
    )
}

pub(in crate::tests) fn integer_exact_cast_parameter_plan(
    parameter_types: &[ScalarType],
    operand_parameter: usize,
    target_type: IntegerType,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, operand_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(source_type) = function.parameters[operand_parameter].scalar_type
    else {
        panic!("integer exact-cast fixture requires an integer operand")
    };
    let result = ValueId::new(4_401).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::IntegerExactCast {
            psi_operation: OperationId::new(4_400).unwrap(),
            obligation: ObligationId::new(4_402).unwrap(),
            result,
            source_type,
            target_type,
            operand,
        },
    );
    finish_integer_expression(function, result, target_type);
    plan
}

pub(in crate::tests) fn uniform_integer_exact_cast_plan(
    source_type: IntegerType,
    target_type: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_exact_cast_parameter_plan(
        &vec![ScalarType::Integer(source_type); parameter_count],
        parameter_count - 1,
        target_type,
    )
}

fn finish_boolean_expression(function: &mut AbstractFunction, value: ValueId) {
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

fn finish_integer_expression(
    function: &mut AbstractFunction,
    value: ValueId,
    integer: IntegerType,
) {
    let AbstractOperation::Return {
        value: returned,
        scalar_type,
        ..
    } = &mut function.operations[1]
    else {
        unreachable!()
    };
    *returned = value;
    *scalar_type = ScalarType::Integer(integer);
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Integer(integer),
    });
}
