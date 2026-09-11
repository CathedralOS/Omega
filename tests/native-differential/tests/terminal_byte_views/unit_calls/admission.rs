//! Unit result absence, Boolean inputs, and pointer custody are independently replayed.
use calling_conventions::{CallSignature, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstructionKind};
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ScalarType, ValueId};
use target::NativeTarget;
use target_operations::{TargetUnitOperation, TargetUnitScalarArgumentSource};
use target_operations_to_selected_instructions::{
    legalize_target_operations, validate_legalized_operations,
};

#[test]
fn unit_call_receiving_rejects_changed_boolean_result_and_pointer_contracts() {
    let module = super::unit_call_module();
    let proof = super::super::fixtures::byte_view_read_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = super::super::byte_view_target(&module, &proof, target);
        let admitted = legalize_target_operations(
            compiled.target_operations(),
            compiled.optimized().plan(),
            compiled.optimized().unit(),
        )
        .unwrap();
        let entry = admitted
            .plan()
            .scalar_functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        assert!(entry.call_plan.result.is_none());
        let unit_calls = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(instruction.kind, LegalizedScalarInstructionKind::Call(_))
            })
            .collect::<Vec<_>>();
        assert_eq!(unit_calls.len(), 2);
        for instruction in unit_calls {
            let LegalizedScalarInstructionKind::Call(call) = &instruction.kind else {
                unreachable!()
            };
            assert!(instruction.result.is_none());
            assert!(call.call_plan.result.is_none());
            assert!(call.result_placement.is_none());
            let LegalizedScalarArgument::Scalar { source, .. } = call.arguments[1] else {
                panic!("Boolean scalar argument")
            };
            assert_eq!(
                source,
                ValueId::new(if instruction.operation == OperationId::new(205).unwrap() {
                    204
                } else {
                    206
                })
                .unwrap()
            );
        }
        for corruption in 0..9 {
            let mut native = compiled.target_operations().clone();
            let entry = native
                .functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let body = &mut entry.graph;
            let operation = if corruption == 8 { 207 } else { 205 };
            let call = body.blocks.iter_mut().flat_map(|block| &mut block.operations).find(|candidate| matches!(candidate, TargetUnitOperation::Call { psi_operation, .. } if *psi_operation == OperationId::new(operation).unwrap())).unwrap();
            let TargetUnitOperation::Call {
                scalar_arguments,
                arguments,
                call_plan,
                ..
            } = call
            else {
                unreachable!()
            };
            match corruption {
                0 => scalar_arguments[1].source = scalar_arguments[0].source,
                1 => scalar_arguments[1].parameter_index = 0,
                2 => scalar_arguments[1].placement.shape = ValueShape::integer(8, 8),
                3 => {
                    let TargetUnitScalarArgumentSource::Parameter { scalar_type, .. } =
                        &mut scalar_arguments[1].source
                    else {
                        panic!("Bool parameter")
                    };
                    *scalar_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
                }
                4 => call_plan.result = Some(call_plan.parameters[0].clone()),
                5 => arguments[0].source = call_plan.parameters[0].clone().into(),
                6 => {
                    let copied = evaluate_call_plan(
                        call_plan.policy,
                        &CallSignature {
                            parameters: vec![
                                ValueShape::integer(8, 8),
                                ValueShape::integer(1, 1),
                                ValueShape::integer(16, 8),
                            ],
                            result: None,
                        },
                    )
                    .unwrap();
                    arguments[0].shape = copied.parameters[2].shape;
                    arguments[0].destination = copied.parameters[2].clone();
                    *call_plan = copied;
                }
                7 => call_plan.parameters.swap(0, 1),
                8 => {
                    let TargetUnitScalarArgumentSource::BooleanImmediate { value, .. } =
                        &mut scalar_arguments[1].source
                    else {
                        panic!("Bool constant")
                    };
                    *value = true;
                }
                _ => unreachable!(),
            }
            assert!(
                legalize_target_operations(
                    &native,
                    compiled.optimized().plan(),
                    compiled.optimized().unit()
                )
                .is_err(),
                "Unit target corruption {corruption} on {target:?}"
            );
        }
        let scalar_result = admitted
            .plan()
            .scalar_functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|instruction| instruction.operation == OperationId::new(105).unwrap())
            .unwrap()
            .result;
        assert!(
            scalar_result.is_some(),
            "discarded reader result is still a real scalar definition"
        );
        for corruption in 0..5 {
            let mut proposed = admitted.plan().clone();
            let entry = proposed
                .scalar_functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let instruction = entry
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| instruction.operation == OperationId::new(205).unwrap())
                .unwrap();
            let LegalizedScalarInstructionKind::Call(call) = &mut instruction.kind else {
                panic!("Unit call")
            };
            match corruption {
                0 => instruction.result = scalar_result,
                1 => call.result_placement = Some(call.call_plan.parameters[0].clone()),
                2 => call.call_plan.result = Some(call.call_plan.parameters[0].clone()),
                3 => {
                    let LegalizedScalarArgument::Scalar { source, .. } = &mut call.arguments[1]
                    else {
                        panic!("Bool parameter")
                    };
                    *source = ValueId::new(206).unwrap();
                }
                4 => call.arguments.swap(0, 1),
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(
                    compiled.target_operations(),
                    compiled.optimized().plan(),
                    compiled.optimized().unit(),
                    proposed
                )
                .is_err(),
                "Unit legalized corruption {corruption} on {target:?}"
            );
        }
    }
}
