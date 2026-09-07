//! Windows register-call execution and independently replayed shadow-area geometry.

use crate::tests::*;

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod execution;
mod frame_replay;

fn artifact(argument_count: usize) -> (Vec<u8>, Vec<u8>) {
    scalar_call_unit_artifact_with(|module| {
        let callee = &mut module.machines[1];
        let scalar_type = callee.parameters[0].scalar_type;
        callee.parameters = (0..argument_count)
            .map(|index| ValueDeclaration {
                id: ValueId::new(26_000 + index as u64).unwrap(),
                scalar_type,
            })
            .collect();
        callee.blocks.truncate(1);
        callee.blocks[0].operations.clear();
        let returned = callee
            .parameters
            .last()
            .map(|parameter| parameter.id)
            .unwrap_or_else(|| {
                let value = ValueId::new(26_100).unwrap();
                callee.blocks[0].operations.push(Operation {
                    id: OperationId::new(26_101).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value,
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(19),
                    },
                });
                value
            });
        callee.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(26_102).unwrap(),
            value: returned,
            cleanup_actions: Vec::new(),
        };
        let left = ValueId::new(SCALAR_CALL_UNIT_LEFT).unwrap();
        let right = ValueId::new(SCALAR_CALL_UNIT_RIGHT).unwrap();
        let first = ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap();
        let second = ValueId::new(SCALAR_CALL_UNIT_SECOND_RESULT).unwrap();
        let arguments = [
            [left, right, left, right],
            [right, left, right, left],
            [first, second, first, second],
        ];
        for (operation, arguments) in module.machines[0].blocks[0].operations[2..]
            .iter_mut()
            .zip(arguments)
        {
            let OperationKind::Call {
                arguments: actual, ..
            } = &mut operation.kind
            else {
                unreachable!()
            };
            *actual = arguments[..argument_count].to_vec();
        }
    })
}

fn physical(
    argument_count: usize,
    selections: &OptimizationSelections,
) -> StagedOptimizedVerifiedPhysicalPipeline {
    let (semantic, proof) = artifact(argument_count);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(selections),
    )
    .unwrap();
    stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::windows_x64(),
        &[],
    )
    .unwrap()
}

#[test]
fn windows_register_calls_execute_validated_text_and_shadow_area_stress() {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    execution::run();
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    eprintln!("SKIP: Windows x86-64 is required for Microsoft register-call execution");
}
