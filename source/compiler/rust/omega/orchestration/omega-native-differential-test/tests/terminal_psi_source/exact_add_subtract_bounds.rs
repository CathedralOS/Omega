use super::*;

#[test]
fn checked_source_exact_add_uses_known_addend_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-addend exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_known_right")
        .expect("known-addend exact addition should use its path bound");
    let add_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }))
        .expect("proof-gated exact addition remains explicit terminal work");
    let OperationKind::ExactIntegerAdd { obligation, .. } = add_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&add_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-add semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-add proof");
    let module = decode_module(&semantic).expect("decode exact-add semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_add_proof = decode_proof_bundle(&proof).expect("decode exact-add proof");
    missing_add_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(&module, &missing_add_proof, &AdmissionProfile::default()),
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
        .expect("verified exact addition should interpret")
    };
    assert_eq!(
        execute(4_294_967_290).value(),
        TerminalExecutionResult::Scalar(argument(4_294_967_295))
    );
    assert_eq!(
        execute(4_294_967_291).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact addition should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerAdd {
                    obligation: retained,
                    ..
                } if *retained == obligation
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact addition should select");
        let assigned = assign_registers(&target_operations).expect("exact-add homes should assign");
        emit_machine_code(&assigned).expect("exact addition should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-add host selection");
        let assigned = assign_registers(&target_operations).expect("exact-add host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-add host emission");
        let object = build_terminal_object_artifact(&machine_code).expect("exact-add host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 100, 0), 105);
        assert_eq!(
            run_host_machine_code_with_two_u64(entry, 4_294_967_291, 0),
            0
        );
    }
}

#[test]
fn checked_source_exact_add_uses_joint_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("joint-bound exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_runtime_bound")
        .expect("joint-bound exact addition should use its path proposition");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let operations = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let subtract_obligation = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerSubtract { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("the bound subtraction remains explicit proof-gated work");
    let add_obligation = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ExactIntegerAdd { obligation, .. } => Some(obligation),
            _ => None,
        })
        .expect("the joint addition remains explicit proof-gated work");

    let semantic = encode_module(&lowered.semantic_module).expect("joint-bound semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("joint-bound proof");
    let module = decode_module(&semantic).expect("decode joint-bound semantics");
    for missing in [subtract_obligation, add_obligation] {
        let mut incomplete = decode_proof_bundle(&proof).expect("decode joint-bound proof");
        incomplete
            .evidence
            .retain(|evidence| evidence.obligation != missing);
        assert!(matches!(
            verify_module(&module, &incomplete, &AdmissionProfile::default()),
            Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
                if obligation == missing
        ));
    }
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
        .expect("verified joint-bound exact addition should interpret")
    };
    assert_eq!(
        execute(20, 22).value(),
        TerminalExecutionResult::Scalar(argument(42))
    );
    assert_eq!(
        execute(4_294_967_285, 10).value(),
        TerminalExecutionResult::Scalar(argument(4_294_967_295))
    );
    assert_eq!(
        execute(4_294_967_295, 1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("joint-bound exact addition should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("joint-bound exact addition should select");
        let assigned = assign_registers(&target_operations)
            .expect("joint-bound exact-add homes should assign");
        emit_machine_code(&assigned).expect("joint-bound exact addition should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("joint-bound exact-add host selection");
        let assigned = assign_registers(&target_operations).expect("joint-bound host homes");
        let machine_code = emit_machine_code(&assigned).expect("joint-bound host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("joint-bound exact-add host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 20, 22), 42);
        assert_eq!(
            run_host_machine_code_with_two_u64(entry, 4_294_967_295, 1),
            0
        );
    }
}

#[test]
fn checked_source_exact_add_uses_signed_nonnegative_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed joint-bound exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_signed_nonnegative_bound")
        .expect("signed joint-bound exact addition should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed joint semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed joint proof");
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
        .expect("verified signed joint-bound exact addition should interpret")
    };
    assert_eq!(
        execute(20, 22).value(),
        TerminalExecutionResult::Scalar(argument(42))
    );
    assert_eq!(
        execute(2_147_483_637, 10).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_647))
    );
    assert_eq!(
        execute(2_147_483_647, 1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(-5, 3).value(),
        TerminalExecutionResult::Scalar(argument(-2))
    );
    assert_eq!(
        execute(20, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed joint-bound exact addition should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed joint-bound exact addition should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed joint-bound exact-add homes should assign");
        emit_machine_code(&assigned).expect("signed joint-bound exact addition should emit");
    }
}

#[test]
fn checked_source_exact_add_uses_signed_nonpositive_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed lower joint-bound exact-add source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_add_signed_nonpositive_bound")
        .expect("signed lower joint-bound exact addition should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed lower joint semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed lower joint proof");
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
        .expect("verified signed lower joint-bound exact addition should interpret")
    };
    assert_eq!(
        execute(-2_147_483_640, -8).value(),
        TerminalExecutionResult::Scalar(argument(i32::MIN as i128))
    );
    assert_eq!(
        execute(i32::MIN as i128, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(5, -3).value(),
        TerminalExecutionResult::Scalar(argument(2))
    );
    assert_eq!(
        execute(i32::MAX as i128, -1).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_646))
    );
    assert_eq!(
        execute(20, 1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed lower joint-bound exact addition should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed lower joint-bound exact addition should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed lower joint-bound exact-add homes should assign");
        emit_machine_code(&assigned).expect("signed lower joint-bound exact addition should emit");
    }
}

#[test]
fn checked_source_exact_subtract_uses_known_subtrahend_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-subtrahend exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_known_right")
        .expect("known-subtrahend exact subtraction should use its path bound");
    let subtract_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. }))
        .expect("proof-gated exact subtraction remains explicit terminal work");
    let OperationKind::ExactIntegerSubtract { obligation, .. } = subtract_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&subtract_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-subtract semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-subtract proof");
    let module = decode_module(&semantic).expect("decode exact-subtract semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_subtract_proof =
        decode_proof_bundle(&proof).expect("decode exact-subtract proof");
    missing_subtract_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_subtract_proof,
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
        .expect("verified exact subtraction should interpret")
    };
    assert_eq!(
        execute(5).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(100).value(),
        TerminalExecutionResult::Scalar(argument(95))
    );
    assert_eq!(
        execute(4).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact subtraction should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerSubtract {
                    obligation: retained,
                    ..
                } if *retained == obligation
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact subtraction should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-subtract homes should assign");
        emit_machine_code(&assigned).expect("exact subtraction should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-subtract host selection");
        let assigned = assign_registers(&target_operations).expect("exact-subtract host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-subtract host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-subtract host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 100, 0), 95);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 4, 0), 0);
    }
}

#[test]
fn checked_source_exact_subtract_uses_joint_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("joint-bound exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_joint_bound")
        .expect("joint-bound exact subtraction should use its path proposition");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("joint subtract semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("joint subtract proof");
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
        .expect("verified joint-bound exact subtraction should interpret")
    };
    assert_eq!(
        execute(42, 20).value(),
        TerminalExecutionResult::Scalar(argument(22))
    );
    assert_eq!(
        execute(u32::MAX as u128, u32::MAX as u128).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(0, 0).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(20, 21).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("joint-bound exact subtraction should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("joint-bound exact subtraction should select");
        let assigned = assign_registers(&target_operations)
            .expect("joint-bound exact-subtract homes should assign");
        emit_machine_code(&assigned).expect("joint-bound exact subtraction should emit");
    }
}

#[test]
fn checked_source_exact_subtract_uses_signed_nonnegative_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed joint-bound exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_signed_nonnegative_bound")
        .expect("signed joint-bound exact subtraction should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed subtract semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed subtract proof");
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
        .expect("verified signed joint-bound exact subtraction should interpret")
    };
    assert_eq!(
        execute(-2_147_483_640, 8).value(),
        TerminalExecutionResult::Scalar(argument(i32::MIN as i128))
    );
    assert_eq!(
        execute(i32::MIN as i128, 1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(5, 3).value(),
        TerminalExecutionResult::Scalar(argument(2))
    );
    assert_eq!(
        execute(i32::MAX as i128, 0).value(),
        TerminalExecutionResult::Scalar(argument(i32::MAX as i128))
    );
    assert_eq!(
        execute(20, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed joint-bound exact subtraction should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed joint-bound exact subtraction should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed joint-bound exact-subtract homes should assign");
        emit_machine_code(&assigned).expect("signed joint-bound exact subtraction should emit");
    }
}

#[test]
fn checked_source_exact_subtract_uses_signed_nonpositive_runtime_bound() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed upper joint-bound exact-subtract source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_subtract_signed_nonpositive_bound")
        .expect("signed upper joint-bound exact subtraction should use both path propositions");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("signed upper semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed upper proof");
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
        .expect("verified signed upper joint-bound exact subtraction should interpret")
    };
    assert_eq!(
        execute(2_147_483_640, -7).value(),
        TerminalExecutionResult::Scalar(argument(i32::MAX as i128))
    );
    assert_eq!(
        execute(i32::MAX as i128, -1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(5, -3).value(),
        TerminalExecutionResult::Scalar(argument(8))
    );
    assert_eq!(
        execute(i32::MIN as i128, 0).value(),
        TerminalExecutionResult::Scalar(argument(i32::MIN as i128))
    );
    assert_eq!(
        execute(20, 1).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("signed upper joint-bound exact subtraction should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("signed upper joint-bound exact subtraction should select");
        let assigned = assign_registers(&target_operations)
            .expect("signed upper joint-bound exact-subtract homes should assign");
        emit_machine_code(&assigned)
            .expect("signed upper joint-bound exact subtraction should emit");
    }
}

#[test]
fn checked_source_exact_add_and_subtract_use_signed_i64_runtime_bounds() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed i64 runtime-bound add/subtract source canaries should compile");
    let cases: [(&str, &[(i128, i128, i128)]); 2] = [
        (
            "terminal_exact_add_signed_i64_runtime_bounds",
            &[
                (i64::MAX as i128 - 5, 5, i64::MAX as i128),
                (i64::MAX as i128 - 4, 5, 0),
                (i64::MIN as i128 + 5, -5, i64::MIN as i128),
                (i64::MIN as i128 + 4, -5, 0),
                (i64::MIN as i128, 5, i64::MIN as i128 + 5),
                (i64::MAX as i128, -5, i64::MAX as i128 - 5),
                (i64::MAX as i128, 0, i64::MAX as i128),
            ],
        ),
        (
            "terminal_exact_subtract_signed_i64_runtime_bounds",
            &[
                (i64::MIN as i128 + 5, 5, i64::MIN as i128),
                (i64::MIN as i128 + 4, 5, 0),
                (i64::MAX as i128 - 5, -5, i64::MAX as i128),
                (i64::MAX as i128 - 4, -5, 0),
                (i64::MAX as i128, 5, i64::MAX as i128 - 5),
                (i64::MIN as i128, -5, i64::MIN as i128 + 5),
                (i64::MIN as i128, 0, i64::MIN as i128),
            ],
        ),
    ];
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };

    for (machine, machine_cases) in cases {
        let lowered = lower_machine(&checked, machine)
            .expect("signed i64 add/subtract should use its runtime-bound propositions");
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let semantic = encode_module(&lowered.semantic_module).expect("signed i64 semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed i64 proof");
        for &(left, right, expected) in machine_cases {
            let execution = interpret_terminal_artifact_measured(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[argument(left), argument(right)],
            )
            .expect("verified signed i64 add/subtract should interpret");
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(argument(expected))
            );
        }

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed i64 add/subtract should cross Omega");
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("signed i64 add/subtract should select");
            let assigned = assign_registers(&target_operations)
                .expect("signed i64 add/subtract homes should assign");
            emit_machine_code(&assigned).expect("signed i64 add/subtract should emit");
        }
    }
}

#[test]
fn checked_source_exact_arithmetic_uses_unsigned_u64_runtime_bounds() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("unsigned u64 runtime-bound arithmetic source canaries should compile");
    let cases: [(&str, bool, &[(u128, u128, u128)]); 3] = [
        (
            "terminal_exact_add_u64_runtime_bound",
            true,
            &[
                (u64::MAX as u128 - 5, 5, u64::MAX as u128),
                (u64::MAX as u128 - 4, 5, 0),
                (0, u64::MAX as u128, u64::MAX as u128),
            ],
        ),
        (
            "terminal_exact_subtract_u64_joint_bound",
            false,
            &[
                (u64::MAX as u128, u64::MAX as u128, 0),
                (0, 1, 0),
                (u64::MAX as u128, 0, u64::MAX as u128),
            ],
        ),
        (
            "terminal_exact_multiply_u64_joint_bound",
            true,
            &[
                (u64::MAX as u128, 1, u64::MAX as u128),
                (6_148_914_691_236_517_205, 3, u64::MAX as u128),
                (6_148_914_691_236_517_206, 3, 0),
                (20, 0, 0),
            ],
        ),
    ];
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };

    for (machine, passes_maximum, machine_cases) in cases {
        let lowered = lower_machine(&checked, machine)
            .expect("unsigned u64 arithmetic should use its runtime-bound propositions");
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let semantic = encode_module(&lowered.semantic_module).expect("unsigned u64 semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("unsigned u64 proof");
        for &(left, right, expected) in machine_cases {
            let mut arguments = vec![argument(left), argument(right)];
            if passes_maximum {
                arguments.push(argument(u64::MAX as u128));
            }
            let execution = interpret_terminal_artifact_measured(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &arguments,
            )
            .expect("verified unsigned u64 arithmetic should interpret");
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(argument(expected))
            );
        }

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("unsigned u64 arithmetic should cross Omega");
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .expect("unsigned u64 arithmetic should select");
            let assigned = assign_registers(&target_operations)
                .expect("unsigned u64 arithmetic homes should assign");
            emit_machine_code(&assigned).expect("unsigned u64 arithmetic should emit");
        }
    }
}
