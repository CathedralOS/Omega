//! Mixed-call receiving checks retain scalar identity and borrowed-reference custody.

use calling_conventions::{CallSignature, ValueShape, evaluate_call_plan};
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstructionKind};
use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, ValueId};
use target::NativeTarget;
use target_operations::{
    ScalarParameterLocation, TargetIntegerExpression, TargetOperation, TargetScalarExpression,
};
use target_operations_to_selected_instructions::{
    legalize_target_operations, validate_legalized_operations,
};
use terminal_psi::StructuralAccess;

use super::{
    byte_view_target,
    fixtures::{byte_view_read_call_module, byte_view_read_proof},
};

#[test]
fn mixed_call_receiving_rejects_substituted_scalar_and_reference_arguments() {
    let module = byte_view_read_call_module();
    let proof = byte_view_read_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&module, &proof, target);
        legalize_target_operations(
            compiled.target_operations(),
            compiled.optimized().plan(),
            compiled.optimized().unit(),
        )
        .expect("uncorrupted mixed call admits");
        for corruption in 0..10 {
            let mut native = compiled.target_operations().clone();
            let caller = native
                .functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let TargetOperation::ReturnIntegerExpression { expression, .. } = &mut caller.operation
            else {
                panic!("mixed reader caller returns an integer expression")
            };
            let TargetIntegerExpression::StructuralCall {
                arguments,
                structural_arguments,
                call_plan,
                ..
            } = expression
            else {
                panic!("mixed reader caller retains its structural call")
            };
            match corruption {
                0 => {
                    let TargetScalarExpression::Integer {
                        expression: TargetIntegerExpression::Parameter { source_value, .. },
                        ..
                    } = &mut arguments[0].expression
                    else {
                        panic!("runtime index parameter")
                    };
                    *source_value = ValueId::new(106).unwrap();
                }
                1 => {
                    arguments[0].location =
                        ScalarParameterLocation::IncomingStack { byte_offset: 0 }
                }
                2 => {
                    arguments[0].scalar_type =
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
                }
                3 => arguments.clear(),
                4 => structural_arguments[0].source_byte_offset = 8,
                5 => structural_arguments[0].destination = call_plan.parameters[0].clone(),
                6 => structural_arguments[0].source = call_plan.parameters[0].clone().into(),
                7 => {
                    let copied = evaluate_call_plan(
                        call_plan.policy,
                        &CallSignature {
                            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 8)],
                            result: Some(ValueShape::integer(8, 8)),
                        },
                    )
                    .unwrap();
                    structural_arguments[0].shape = copied.parameters[1].shape;
                    structural_arguments[0].destination = copied.parameters[1].clone();
                    *call_plan = copied;
                }
                8 => call_plan.parameters.swap(0, 1),
                9 => structural_arguments[0].place = PlaceId::new(3).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                legalize_target_operations(
                    &native,
                    compiled.optimized().plan(),
                    compiled.optimized().unit()
                )
                .is_err(),
                "target substitution {corruption} on {target:?}"
            );
        }
    }
}

#[test]
fn mixed_call_replay_rejects_changed_scalar_and_reference_custody() {
    let module = byte_view_read_call_module();
    let proof = byte_view_read_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&module, &proof, target);
        let native = compiled.target_operations();
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized().unit();
        let admitted = legalize_target_operations(native, abstracted, unit).unwrap();
        for corruption in 0..8 {
            let mut proposed = admitted.plan().clone();
            let caller = proposed
                .scalar_functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let call = caller
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find_map(|instruction| match &mut instruction.kind {
                    LegalizedScalarInstructionKind::Call(call) => Some(call),
                    _ => None,
                })
                .expect("mixed call instruction");
            match corruption {
                0 => {
                    let LegalizedScalarArgument::Scalar { source, .. } = &mut call.arguments[0]
                    else {
                        panic!("scalar prefix")
                    };
                    *source = ValueId::new(106).unwrap();
                }
                1 => {
                    let LegalizedScalarArgument::Scalar { placement, .. } = &mut call.arguments[0]
                    else {
                        panic!("scalar prefix")
                    };
                    *placement = call.call_plan.parameters[1].clone();
                }
                2 => {
                    call.arguments.pop();
                }
                3 => call.arguments.swap(0, 1),
                4 => {
                    let LegalizedScalarArgument::Structural { semantic, target } =
                        &mut call.arguments[1]
                    else {
                        panic!("structural suffix")
                    };
                    semantic.access = StructuralAccess::MutableBorrow;
                    target.access = semantic.access;
                }
                5 => {
                    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[1]
                    else {
                        panic!("structural suffix")
                    };
                    target.source = call.result_placement.clone().unwrap().into();
                }
                6 => call.call_plan.parameters.swap(0, 1),
                7 => {
                    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[1]
                    else {
                        panic!("structural suffix")
                    };
                    target.destination = call.call_plan.parameters[0].clone();
                }
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(native, abstracted, unit, proposed).is_err(),
                "legalized substitution {corruption} on {target:?}"
            );
        }
    }
}
