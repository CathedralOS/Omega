use super::*;

#[test]
fn checked_source_exact_multiply_uses_known_factor_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-factor exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_known_right")
        .expect("known-factor exact multiplication should use its path bound");
    let multiply_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerMultiply { .. }))
        .expect("proof-gated exact multiplication remains explicit terminal work");
    let OperationKind::ExactIntegerMultiply { obligation, .. } = multiply_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&multiply_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-multiply proof");
    let module = decode_module(&semantic).expect("decode exact-multiply semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_multiply_proof =
        decode_proof_bundle(&proof).expect("decode exact-multiply proof");
    missing_multiply_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_multiply_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(0)],
        )
        .expect("verified exact multiplication should interpret")
    };
    assert_eq!(
        execute(858_993_459).value(),
        TerminalExecutionResult::Scalar(argument(4_294_967_295))
    );
    assert_eq!(
        execute(100).value(),
        TerminalExecutionResult::Scalar(argument(500))
    );
    assert_eq!(
        execute(858_993_460).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact multiplication should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerMultiply {
                    obligation: retained,
                    ..
                } if *retained == obligation
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact multiplication should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("exact multiplication should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-multiply host selection");
        let assigned = assign_registers(&target_operations).expect("exact-multiply host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-multiply host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-multiply host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 100, 0, 500));
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            858_993_460,
            0,
            0
        ));
    }
}

#[test]
fn checked_source_exact_multiply_uses_joint_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("joint-bound exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_joint_bound")
        .expect("joint-bound exact multiplication should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("joint multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("joint multiply proof");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified joint-bound exact multiplication should interpret")
    };
    assert_eq!(
        execute(21, 2).value(),
        TerminalExecutionResult::Scalar(argument(42))
    );
    assert_eq!(
        execute(u32::MAX as u128, 1).value(),
        TerminalExecutionResult::Scalar(argument(u32::MAX as u128))
    );
    assert_eq!(
        execute(65_535, 65_537).value(),
        TerminalExecutionResult::Scalar(argument(u32::MAX as u128))
    );
    assert_eq!(
        execute(u32::MAX as u128, 2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, 0).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("joint-bound exact multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("joint-bound exact multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("joint-bound exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("joint-bound exact multiplication should emit");
    }
}

#[test]
fn checked_source_exact_multiply_uses_signed_positive_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed joint-bound exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_signed_positive_bound")
        .expect("signed joint-bound exact multiplication should use all path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed multiply proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed joint-bound exact multiplication should interpret")
    };
    assert_eq!(
        execute(21, 2).value(),
        TerminalExecutionResult::Scalar(argument(42))
    );
    assert_eq!(
        execute(-1_073_741_824, 2).value(),
        TerminalExecutionResult::Scalar(argument(i32::MIN as i128))
    );
    assert_eq!(
        execute(715_827_882, 3).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_646))
    );
    assert_eq!(
        execute(-1_073_741_825, 2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(1_073_741_824, 2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, 0).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed joint-bound exact multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed joint-bound exact multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed joint-bound exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("signed joint-bound exact multiplication should emit");
    }
}

#[test]
fn checked_source_exact_multiply_uses_signed_negative_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("negative signed joint-bound exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_signed_negative_bound").expect(
        "negative signed joint-bound exact multiplication should use all path propositions",
    );
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("negative multiply semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("negative multiply proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified negative signed joint-bound multiplication should interpret")
    };
    assert_eq!(
        execute(-1_073_741_823, -2).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_646))
    );
    assert_eq!(
        execute(1_073_741_824, -2).value(),
        TerminalExecutionResult::Scalar(argument(i32::MIN as i128))
    );
    assert_eq!(
        execute(-715_827_882, -3).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_646))
    );
    assert_eq!(
        execute(715_827_882, -3).value(),
        TerminalExecutionResult::Scalar(argument(-2_147_483_646))
    );
    assert_eq!(
        execute(-1_073_741_824, -2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(1_073_741_825, -2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, 0).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("negative signed joint-bound multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("negative signed joint-bound multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("negative signed joint-bound exact-multiply homes should assign");
        emit_machine_code(&assigned)
            .expect("negative signed joint-bound exact multiplication should emit");
    }
}

#[test]
fn checked_source_exact_multiply_uses_signed_runtime_negation_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("runtime-negation exact-multiply source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_multiply_signed_negation_bound")
        .expect("runtime-negation exact multiplication should use all path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("runtime-negation semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("runtime-negation proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified runtime-negation exact multiplication should interpret")
    };
    assert_eq!(
        execute(-2_147_483_647, -1).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_647))
    );
    assert_eq!(
        execute(2_147_483_647, -1).value(),
        TerminalExecutionResult::Scalar(argument(-2_147_483_647))
    );
    assert_eq!(
        execute(0, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(i32::MIN as i128, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, -2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, 0).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("runtime-negation exact multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("runtime-negation exact multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("runtime-negation exact-multiply homes should assign");
        emit_machine_code(&assigned).expect("runtime-negation exact multiplication should emit");
    }
}

#[test]
fn checked_source_exact_multiply_uses_all_signed_i64_runtime_bounds() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed i64 runtime-bound exact-multiply source canary should compile");
    let lowered = lower_machine(
        &checked,
        "terminal_exact_multiply_signed_i64_runtime_bounds",
    )
    .expect("signed i64 exact multiplication should use every runtime-factor proof form");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed i64 semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed i64 proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execute = |left, right| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(left), argument(right)],
        )
        .expect("verified signed i64 runtime-bound multiplication should interpret")
    };

    assert_eq!(
        execute(i64::MIN as i128 + 1, -1).value(),
        TerminalExecutionResult::Scalar(argument(i64::MAX as i128))
    );
    assert_eq!(
        execute(i64::MIN as i128, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    assert_eq!(
        execute(-4_611_686_018_427_387_904, 2).value(),
        TerminalExecutionResult::Scalar(argument(i64::MIN as i128))
    );
    assert_eq!(
        execute(3_074_457_345_618_258_602, 3).value(),
        TerminalExecutionResult::Scalar(argument(9_223_372_036_854_775_806))
    );
    assert_eq!(
        execute(-4_611_686_018_427_387_905, 2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(4_611_686_018_427_387_904, 2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    assert_eq!(
        execute(-4_611_686_018_427_387_903, -2).value(),
        TerminalExecutionResult::Scalar(argument(9_223_372_036_854_775_806))
    );
    assert_eq!(
        execute(4_611_686_018_427_387_904, -2).value(),
        TerminalExecutionResult::Scalar(argument(i64::MIN as i128))
    );
    assert_eq!(
        execute(-4_611_686_018_427_387_904, -2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(4_611_686_018_427_387_905, -2).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, 0).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed i64 runtime-bound multiplication should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed i64 runtime-bound multiplication should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed i64 runtime-bound multiply homes should assign");
        emit_machine_code(&assigned).expect("signed i64 runtime-bound multiplication should emit");
    }
}
