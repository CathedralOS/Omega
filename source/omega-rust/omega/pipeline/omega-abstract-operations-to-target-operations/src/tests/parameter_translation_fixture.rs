use super::*;

pub(super) fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("test integer type")
}

pub(super) fn parameter_return_plan(
    parameter_types: &[ScalarType],
    returned_parameter: usize,
) -> AbstractOperationPlan {
    let machine = MachineId::new(3_001).unwrap();
    let entry = BlockId::new(3_002).unwrap();
    let result_value = ValueId::new(3_003).unwrap();
    let return_edge = EdgeId::new(3_004).unwrap();
    let parameters = parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| AbstractParameter {
            value: ValueId::new(3_100 + index as u64).unwrap(),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let scalar_type = parameter_types[returned_parameter];
    AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry,
            parameters: parameters.clone(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result_value,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: entry,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![AbstractOperation::Return {
                psi_edge: return_edge,
                result: result_value,
                value: parameters[returned_parameter].value,
                scalar_type,
                cleanup_actions: Vec::new(),
            }],
        }],
    }
}

pub(super) fn uniform_integer_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    parameter_return_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 1,
    )
}

pub(super) fn uniform_boolean_plan(parameter_count: usize) -> AbstractOperationPlan {
    parameter_return_plan(
        &vec![ScalarType::Boolean; parameter_count],
        parameter_count - 1,
    )
}

pub(super) fn boolean_not_parameter_plan(
    parameter_types: &[ScalarType],
    operand_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, operand_parameter);
    let function = &mut plan.functions[0];
    let not_result = ValueId::new(3_701).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::BooleanNot {
            psi_operation: OperationId::new(3_700).unwrap(),
            result: not_result,
            operand,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = not_result;
    *scalar_type = ScalarType::Boolean;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    plan
}

pub(super) fn uniform_boolean_not_plan(parameter_count: usize) -> AbstractOperationPlan {
    boolean_not_parameter_plan(
        &vec![ScalarType::Boolean; parameter_count],
        parameter_count - 1,
    )
}

pub(super) fn integer_bitwise_not_parameter_plan(
    parameter_types: &[ScalarType],
    operand_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, operand_parameter);
    let function = &mut plan.functions[0];
    let ScalarType::Integer(scalar_type) = function.parameters[operand_parameter].scalar_type
    else {
        panic!("integer bitwise-not fixture requires an integer operand")
    };
    let bitwise_not_result = ValueId::new(4_201).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::IntegerBitwiseNot {
            psi_operation: OperationId::new(4_200).unwrap(),
            result: bitwise_not_result,
            scalar_type,
            operand,
        },
    );
    let AbstractOperation::Return {
        value,
        scalar_type: return_type,
        ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = bitwise_not_result;
    *return_type = ScalarType::Integer(scalar_type);
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Integer(scalar_type),
    });
    plan
}

pub(super) fn uniform_integer_bitwise_not_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_bitwise_not_parameter_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 1,
    )
}

pub(super) fn integer_widen_parameter_plan(
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
    let widen_result = ValueId::new(4_301).unwrap();
    let operand = function.parameters[operand_parameter].value;
    function.operations.insert(
        0,
        AbstractOperation::IntegerWiden {
            psi_operation: OperationId::new(4_300).unwrap(),
            result: widen_result,
            source_type,
            target_type,
            operand,
        },
    );
    let AbstractOperation::Return {
        value,
        scalar_type: return_type,
        ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = widen_result;
    *return_type = ScalarType::Integer(target_type);
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Integer(target_type),
    });
    plan
}

pub(super) fn uniform_integer_widen_plan(
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

pub(super) fn boolean_equal_parameters_plan(
    parameter_types: &[ScalarType],
    left_parameter: usize,
    right_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, left_parameter);
    let function = &mut plan.functions[0];
    let equal_result = ValueId::new(3_801).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::BooleanEqual {
            psi_operation: OperationId::new(3_800).unwrap(),
            result: equal_result,
            left: function.parameters[left_parameter].value,
            right: function.parameters[right_parameter].value,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = equal_result;
    *scalar_type = ScalarType::Boolean;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    plan
}

pub(super) fn uniform_boolean_equal_plan(parameter_count: usize) -> AbstractOperationPlan {
    boolean_equal_parameters_plan(
        &vec![ScalarType::Boolean; parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

pub(super) fn integer_equal_parameters_plan(
    parameter_types: &[ScalarType],
    left_parameter: usize,
    right_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, left_parameter);
    let function = &mut plan.functions[0];
    let equal_result = ValueId::new(3_901).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::IntegerEqual {
            psi_operation: OperationId::new(3_900).unwrap(),
            result: equal_result,
            left: function.parameters[left_parameter].value,
            right: function.parameters[right_parameter].value,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = equal_result;
    *scalar_type = ScalarType::Boolean;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    plan
}

pub(super) fn uniform_integer_equal_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_equal_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

pub(super) fn integer_less_than_parameters_plan(
    parameter_types: &[ScalarType],
    left_parameter: usize,
    right_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, left_parameter);
    let function = &mut plan.functions[0];
    let less_than_result = ValueId::new(4_001).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(4_000).unwrap(),
            result: less_than_result,
            left: function.parameters[left_parameter].value,
            right: function.parameters[right_parameter].value,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = less_than_result;
    *scalar_type = ScalarType::Boolean;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    plan
}

pub(super) fn uniform_integer_less_than_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_less_than_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}

pub(super) fn integer_less_or_equal_parameters_plan(
    parameter_types: &[ScalarType],
    left_parameter: usize,
    right_parameter: usize,
) -> AbstractOperationPlan {
    let mut plan = parameter_return_plan(parameter_types, left_parameter);
    let function = &mut plan.functions[0];
    let less_or_equal_result = ValueId::new(4_101).unwrap();
    function.operations.insert(
        0,
        AbstractOperation::IntegerLessOrEqual {
            psi_operation: OperationId::new(4_100).unwrap(),
            result: less_or_equal_result,
            left: function.parameters[left_parameter].value,
            right: function.parameters[right_parameter].value,
        },
    );
    let AbstractOperation::Return {
        value, scalar_type, ..
    } = &mut function.operations[1]
    else {
        unreachable!("fixture ends in return")
    };
    *value = less_or_equal_result;
    *scalar_type = ScalarType::Boolean;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(3_003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    plan
}

pub(super) fn uniform_integer_less_or_equal_plan(
    integer: IntegerType,
    parameter_count: usize,
) -> AbstractOperationPlan {
    integer_less_or_equal_parameters_plan(
        &vec![ScalarType::Integer(integer); parameter_count],
        parameter_count - 2,
        parameter_count - 1,
    )
}
