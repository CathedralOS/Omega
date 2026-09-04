use super::*;

fn mixed_fixed_integer_plan() -> (
    AbstractOperationPlan,
    Vec<AbstractParameter>,
    AbstractResult,
) {
    let machine = MachineId::new(701).unwrap();
    let scalar_types = [
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    ];
    let parameters = scalar_types
        .into_iter()
        .enumerate()
        .map(|(index, integer_type)| AbstractParameter {
            value: ValueId::new(710 + index as u64).unwrap(),
            scalar_type: ScalarType::Integer(integer_type),
        })
        .collect::<Vec<_>>();
    let result = AbstractResult {
        value: ValueId::new(720).unwrap(),
        scalar_type: parameters.last().unwrap().scalar_type,
    };
    (
        AbstractOperationPlan {
            psi: identity(),
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: BlockId::new(701).unwrap(),
                parameters: parameters.clone(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(result),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![AbstractOperation::Return {
                    psi_edge: EdgeId::new(701).unwrap(),
                    result: result.value,
                    value: parameters.last().unwrap().value,
                    scalar_type: result.scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            }],
        },
        parameters,
        result,
    )
}

#[test]
fn fixed_integer_scalar_abi_binds_ordered_values_types_and_canonical_placements() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
    ] {
        let (plan, parameters, result) = mixed_fixed_integer_plan();
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let abi = lowered.functions[0]
            .fixed_integer_scalar_abi
            .as_ref()
            .expect("eligible function ABI");
        let signature = CallSignature {
            parameters: parameters
                .iter()
                .map(|parameter| {
                    let ScalarType::Integer(integer_type) = parameter.scalar_type else {
                        unreachable!()
                    };
                    let bytes = integer_type.bits().div_ceil(8);
                    ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
                })
                .collect(),
            result: Some(ValueShape::integer(8, 8)),
        };
        let canonical = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
            .expect("canonical call plan");
        assert_eq!(abi.call_plan, canonical);
        assert_eq!(abi.parameters.len(), parameters.len());
        for (index, (actual, expected)) in abi.parameters.iter().zip(&parameters).enumerate() {
            let ScalarType::Integer(expected_type) = expected.scalar_type else {
                unreachable!()
            };
            assert_eq!(actual.value, expected.value);
            assert_eq!(actual.scalar_type, expected_type);
            assert_eq!(actual.placement, canonical.parameters[index]);
        }
        let ScalarType::Integer(result_type) = result.scalar_type else {
            unreachable!()
        };
        assert_eq!(abi.result.value, result.value);
        assert_eq!(abi.result.scalar_type, result_type);
        assert_eq!(abi.result.placement, canonical.result.clone().unwrap());
    }
}

#[test]
fn non_fixed_and_non_integer_shapes_publish_no_scalar_abi() {
    let assert_none = |plan: &AbstractOperationPlan| {
        let lowered = lower_to_target_operations(plan, NativeTarget::linux_x64()).unwrap();
        assert_eq!(lowered.functions[0].fixed_integer_scalar_abi, None);
    };

    let (mut address, _, _) = mixed_fixed_integer_plan();
    let address_type = IntegerType::address(64).unwrap();
    address.functions[0].parameters[0].scalar_type = ScalarType::Integer(address_type);
    assert_none(&address);

    let (mut boolean, _, _) = mixed_fixed_integer_plan();
    boolean.functions[0].parameters[0].scalar_type = ScalarType::Boolean;
    assert_none(&boolean);
}

#[test]
fn unit_and_unsupported_width_functions_publish_no_scalar_abi() {
    let (mut unit, _, _) = mixed_fixed_integer_plan();
    unit.functions[0].parameters.clear();
    unit.functions[0].result = AbstractFunctionResult::Unit;
    unit.functions[0].block_entries = vec![AbstractBlockEntry {
        block: unit.functions[0].entry,
        parameters: Vec::new(),
        operation_offset: 0,
    }];
    unit.functions[0].operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(701).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    let lowered = lower_to_target_operations(&unit, NativeTarget::linux_x64()).unwrap();
    assert_eq!(lowered.functions[0].fixed_integer_scalar_abi, None);

    let machine = MachineId::new(730).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 24).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let constant = ValueId::new(731).unwrap();
    let result = ValueId::new(732).unwrap();
    let unsupported = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(730).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: vec![
                AbstractOperation::IntegerConstant {
                    psi_operation: OperationId::new(731).unwrap(),
                    result: constant,
                    scalar_type,
                    value: IntegerValue::Unsigned(7),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(730).unwrap(),
                    result,
                    value: constant,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let lowered = lower_to_target_operations(&unsupported, NativeTarget::linux_x64()).unwrap();
    assert_eq!(lowered.functions[0].fixed_integer_scalar_abi, None);
}
