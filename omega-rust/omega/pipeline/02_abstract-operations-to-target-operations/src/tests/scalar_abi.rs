use super::super::{TargetLoweringRequest, lower_to_target_operations};
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, BlockId, CallSignature,
    CallingPolicy, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, NativeTarget,
    OperationId, ScalarType, ValueId, ValueShape, evaluate_call_plan, identity,
};
use target_operations::{ScalarAbiValue, ScalarFunctionAbi};
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
            structural_types: Vec::new().into(),
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
                block_entries: vec![AbstractBlockEntry {
                    block: BlockId::new(701).unwrap(),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operation_offset: 0,
                }],
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
fn scalar_abi_binds_ordered_values_types_and_canonical_placements() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
    ] {
        let (plan, parameters, result) = mixed_fixed_integer_plan();
        let lowered =
            lower_to_target_operations(&plan, TargetLoweringRequest::new(target)).unwrap();
        let abi = lowered.functions[0]
            .scalar_abi
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
            assert_eq!(actual.value, expected.value);
            assert_eq!(actual.scalar_type, expected.scalar_type);
            assert_eq!(actual.placement, canonical.parameters[index]);
        }
        assert_eq!(abi.result.value, result.value);
        assert_eq!(abi.result.scalar_type, result.scalar_type);
        assert_eq!(abi.result.placement, canonical.result.clone().unwrap());
    }
}

#[test]
fn address_shapes_reject_and_boolean_parameters_keep_their_type() {
    let (mut address, _, _) = mixed_fixed_integer_plan();
    let address_type = IntegerType::address(64).unwrap();
    address.functions[0].parameters[0].scalar_type = ScalarType::Integer(address_type);
    assert!(
        lower_to_target_operations(
            &address,
            TargetLoweringRequest::new(NativeTarget::linux_x64())
        )
        .is_err()
    );

    let (mut boolean, _, _) = mixed_fixed_integer_plan();
    boolean.functions[0].parameters[0].scalar_type = ScalarType::Boolean;
    let lowered = lower_to_target_operations(
        &boolean,
        TargetLoweringRequest::new(NativeTarget::linux_x64()),
    )
    .unwrap();
    let abi = lowered.functions[0]
        .scalar_abi
        .as_ref()
        .expect("Boolean parameter ABI");
    assert_eq!(abi.parameters[0].scalar_type, ScalarType::Boolean);
    assert_eq!(abi.parameters[0].placement.shape, ValueShape::integer(1, 1));
}

#[test]
fn unit_and_unsupported_width_functions_publish_no_scalar_abi() {
    let (mut unit, _, _) = mixed_fixed_integer_plan();
    unit.functions[0].parameters.clear();
    unit.functions[0].result = AbstractFunctionResult::Unit;
    unit.functions[0].block_entries = vec![AbstractBlockEntry {
        structural_parameters: Vec::new(),
        block: unit.functions[0].entry,
        parameters: Vec::new(),
        operation_offset: 0,
    }];
    unit.functions[0].operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(701).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    let lowered =
        lower_to_target_operations(&unit, TargetLoweringRequest::new(NativeTarget::linux_x64()))
            .unwrap();
    assert_eq!(lowered.functions[0].scalar_abi, None);

    let machine = MachineId::new(730).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 24).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let constant = ValueId::new(731).unwrap();
    let result = ValueId::new(732).unwrap();
    let unsupported = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: Vec::new().into(),
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
            block_entries: vec![AbstractBlockEntry {
                block: BlockId::new(730).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }],
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
    let lowered = lower_to_target_operations(
        &unsupported,
        TargetLoweringRequest::new(NativeTarget::linux_x64()),
    )
    .unwrap();
    assert_eq!(lowered.functions[0].scalar_abi, None);
}

/// The published scalar ABI is a standalone receiving entrance: an outside
/// caller observes only these rows. Each must re-derive the declared value,
/// type, and canonical placement; a substituted row or plan detail rejects.
#[test]
fn standalone_scalar_entrance_rejects_substituted_rows() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, _, _) = mixed_fixed_integer_plan();
        let lowered =
            lower_to_target_operations(&source, TargetLoweringRequest::new(native)).unwrap();
        crate::validate_abstract_to_target_translation(&source, native, &lowered).unwrap();
        for mutation in 0..8 {
            let mut changed = lowered.clone();
            let function = &mut changed.functions[0];
            let abi = function.scalar_abi.as_mut().unwrap();
            match mutation {
                0 => abi.parameters[0].value = ValueId::new(1).unwrap(),
                1 => abi.parameters[0].scalar_type = ScalarType::Boolean,
                2 => abi.parameters[0].placement = abi.call_plan.parameters[1].clone(),
                3 => {
                    abi.parameters.pop();
                }
                4 => abi.result.value = ValueId::new(1).unwrap(),
                5 => abi.result.scalar_type = ScalarType::Boolean,
                6 => abi.result.placement = abi.call_plan.parameters[0].clone(),
                _ => abi.call_plan.stack_alignment = abi.call_plan.stack_alignment.wrapping_add(8),
            }
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &changed).is_err(),
                "scalar ABI substitution {mutation}"
            );
        }
    }
}

/// A scalar ABI is derivable only for the scalar-only service-free family.
/// A forged entrance on an ineligible function rejects even when its plan is
/// internally consistent.
#[test]
fn standalone_scalar_entrance_rejects_forged_abi_on_ineligible_function() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (mut unit, _, _) = mixed_fixed_integer_plan();
        unit.functions[0].parameters.clear();
        unit.functions[0].result = AbstractFunctionResult::Unit;
        unit.functions[0].operations = vec![AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(701).unwrap(),
            cleanup_actions: Vec::new(),
        }];
        let lowered =
            lower_to_target_operations(&unit, TargetLoweringRequest::new(native)).unwrap();
        assert_eq!(lowered.functions[0].scalar_abi, None);
        crate::validate_abstract_to_target_translation(&unit, native, &lowered).unwrap();

        let forged_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let mut changed = lowered.clone();
        changed.functions[0].scalar_abi = Some(ScalarFunctionAbi {
            result: ScalarAbiValue {
                value: ValueId::new(720).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                placement: forged_plan.result.clone().unwrap(),
            },
            call_plan: forged_plan,
            parameters: Vec::new(),
        });
        assert!(
            crate::validate_abstract_to_target_translation(&unit, native, &changed).is_err(),
            "a Unit-result function cannot publish a scalar receiving entrance"
        );
    }
}
