//! Frame expansion must preserve a source-admitted short branch and both call arms.

use super::*;

fn padded_returning_call_artifact(equal: bool, padding: u32) -> (Vec<u8>, Vec<u8>) {
    let (semantic, proof) = super::control_flow::branch_call_artifact(equal);
    let mut module = terminal_codec::decode_module(&semantic).unwrap();
    let middle = &mut module.machines[1];
    let scalar_type = middle.parameters[0].scalar_type;
    middle.blocks.truncate(3);
    for arm in &mut middle.blocks[1..] {
        let Terminator::Jump {
            edge, arguments, ..
        } = &arm.terminator
        else {
            unreachable!()
        };
        arm.terminator = Terminator::Return {
            edge: *edge,
            value: arguments[0],
            cleanup_actions: Vec::new(),
        };
    }
    // Retained ordinary constants enlarge only the fallthrough arm. Its real
    // call result remains live until return, whose frame epilogue is inserted
    // inside the branch interval. No proof-bearing arithmetic is invented.
    for padding_index in 0..padding {
        let identity = 29_000 + u64::from(padding_index);
        middle.blocks[1].operations.push(Operation {
            id: OperationId::new(identity).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(identity).unwrap(),
                scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0x1234_5678_9abc_0000 + u128::from(padding_index)),
            },
        });
    }
    (terminal_codec::encode_module(&module).unwrap(), proof)
}

fn framed(equal: bool, padding: u32) -> (bool, StagedFunctionFragmentFrameApplication) {
    let (semantic, proof) = padded_returning_call_artifact(equal, padding);
    let interpreted = terminal_interpreter::interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
    )
    .unwrap();
    let expected = if equal { 37 } else { 41 };
    assert!(matches!(interpreted.value(),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(actual), ..
            }
        ) if actual == expected
    ));
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::windows_x64(),
        &[],
    )
    .unwrap();
    let realization = physical.fixed_frame_for_test();
    validate_fixed_frame_function_relative_realization(realization).unwrap();
    let emitted = stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let middle = emitted
        .fragments()
        .functions
        .iter()
        .find(|function| function.machine.get() == 28_101)
        .unwrap();
    let branch = middle
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.branch.is_some())
        .unwrap();
    let short = branch.bytes.len() == 2;
    let branch_identity = branch.instruction;
    let applied = stage_function_fragment_frame_application(emitted).expect(
        "frame expansion must widen an overflowing rel8 without losing admitted source custody",
    );
    validate_function_fragment_frame_application(&applied).unwrap();
    let middle = applied
        .fragments()
        .functions
        .iter()
        .find(|function| function.machine.get() == 28_101)
        .unwrap();
    let branch = middle
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.instruction == branch_identity)
        .unwrap();
    (short && branch.bytes.len() == 6, applied)
}

#[test]
fn framed_rel8_near_limit_preserves_both_returning_call_arms() {
    // Ten retained constants are the measured Windows x64 boundary: frame
    // insertion widens the admitted rel8 branch before either call arm returns.
    let (widened, taken) = framed(true, 10);
    assert!(widened);
    let (widened, untaken) = framed(false, 10);
    assert!(widened);
    for (expected, applied) in [(37, taken), (41, untaken)] {
        let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
        let object = stage_optimized_relocation_free_object_container(text).unwrap();
        validate_optimized_relocation_free_object_container(&object).unwrap();
        let object = std::sync::Arc::new(object);
        let published =
            image_emission::build_function_fragment_object_artifact(object.clone()).unwrap();
        image_emission::validate_function_fragment_object_artifact(&object, &published).unwrap();
        let image = image_emission::emit_executable_image(&published, 3).unwrap();
        image_emission::validate_executable_image(&published, &image).unwrap();
        let record = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
        )
        .unwrap();
        let decoded = image_emission::decode_installation_record(
            &image_emission::encode_installation_record(&record).unwrap(),
        )
        .unwrap();
        image_emission::validate_installation_record(&decoded, &image).unwrap();
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            let text = object.source().text_section();
            let code = super::super::native_execution::Code::new(&text.bytes);
            assert_eq!(
                code.call_scalar(usize::try_from(text.semantic_entry_offset).unwrap(), [0; 4]),
                expected
            );
        }
        #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
        {
            let _ = expected;
            eprintln!(
                "SKIP: Windows x86-64 execution unavailable; artifact and installation replay ran"
            );
        }
    }
}
