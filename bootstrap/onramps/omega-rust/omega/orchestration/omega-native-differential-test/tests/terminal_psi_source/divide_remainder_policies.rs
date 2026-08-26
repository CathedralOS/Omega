use super::*;

#[test]
fn checked_source_exact_divide_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor exact-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_divide_known_right")
        .expect("known nonzero exact division should lower");
    let divide_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerDivide { .. }))
        .expect("proof-gated exact division remains explicit terminal work");
    let OperationKind::ExactIntegerDivide { obligation, .. } = divide_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&divide_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-divide proof");
    let module = decode_module(&semantic).expect("decode exact-divide semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_divide_proof = decode_proof_bundle(&proof).expect("decode exact-divide proof");
    missing_divide_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_divide_proof,
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
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(500), argument(0)],
    )
    .expect("verified exact division should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(100))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact division should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerDivide { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact division should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-divide homes should assign");
        emit_machine_code(&assigned).expect("exact division should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-divide host selection");
        let assigned = assign_registers(&target_operations).expect("exact-divide host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 500, 0, 100));
    }
}

#[test]
fn checked_source_signed_exact_divide_truncates_toward_zero() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed exact-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_signed_divide_known_right")
        .expect("known signed exact division should lower");
    let semantic = encode_module(&lowered.semantic_module).expect("signed exact-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed exact-divide proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(-101), argument(0)],
    )
    .expect("verified signed exact division should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(-50))
    );

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed exact division should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed exact-divide host selection");
        let assigned = assign_registers(&target_operations).expect("signed exact-divide homes");
        let machine_code = emit_machine_code(&assigned).expect("signed exact-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("signed exact-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            (-101_i64) as u64,
            0,
            (-50_i64) as u64,
        ));
    }
}

#[test]
fn checked_source_exact_remainder_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor exact-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_remainder_known_right")
        .expect("known nonzero exact remainder should lower");
    let remainder_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerRemainder { .. }))
        .expect("proof-gated exact remainder remains explicit terminal work");
    let OperationKind::ExactIntegerRemainder { obligation, .. } = remainder_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&remainder_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact-remainder proof");
    let module = decode_module(&semantic).expect("decode exact-remainder semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_remainder_proof =
        decode_proof_bundle(&proof).expect("decode exact-remainder proof");
    missing_remainder_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_remainder_proof,
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
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(503), argument(0)],
    )
    .expect("verified exact remainder should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(3))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact remainder should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerRemainder { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact remainder should select");
        let assigned =
            assign_registers(&target_operations).expect("exact-remainder homes should assign");
        emit_machine_code(&assigned).expect("exact remainder should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact-remainder host selection");
        let assigned = assign_registers(&target_operations).expect("exact-remainder host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact-remainder host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 503, 0, 3));
    }
}

#[test]
fn checked_source_signed_exact_remainder_is_truncating() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed exact-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_signed_remainder_known_right")
        .expect("known signed exact remainder should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed exact-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed exact-remainder proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(-101), argument(0)],
    )
    .expect("verified signed exact remainder should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(-1))
    );

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed exact remainder should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed exact-remainder host selection");
        let assigned = assign_registers(&target_operations).expect("signed exact-remainder homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed exact-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed exact-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            (-101_i64) as u64,
            0,
            (-1_i64) as u64,
        ));
    }
}

#[test]
fn checked_source_wrapping_divide_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor wrapping-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_divide_known_right")
        .expect("known nonzero wrapping division should lower");
    let divide_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::WrappingIntegerDivide { .. }))
        .expect("proof-gated wrapping division remains explicit terminal work");
    let OperationKind::WrappingIntegerDivide { obligation, .. } = divide_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&divide_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("wrapping-divide obligation reconstructs");
    let site = reconstructed
        .iter()
        .find(|site| site.obligation.id == obligation)
        .expect("wrapping-divide obligation is reconstructed");
    let OperationKind::WrappingIntegerDivide { right, .. } = divide_operation.kind else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    assert_eq!(
        site.obligation.proposition,
        psi_core::Proposition::LessOrEqual(
            psi_core::ScalarTerm::integer(u32_type, IntegerValue::Unsigned(1)).unwrap(),
            psi_core::ScalarTerm::value(right, ScalarType::Integer(u32_type)),
        )
    );
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .expect("wrapping-divide certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route
    else {
        panic!("wrapping divide must use a checked certificate")
    };
    assert!(matches!(
        certificate.proof.rule,
        psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
    ));

    let semantic = encode_module(&lowered.semantic_module).expect("wrapping-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("wrapping-divide proof");
    let module = decode_module(&semantic).expect("decode wrapping-divide semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_divide_proof =
        decode_proof_bundle(&proof).expect("decode wrapping-divide proof");
    missing_divide_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let mut corrupt_divide_proof = decode_proof_bundle(&proof).expect("decode wrapping proof");
    let corrupt = corrupt_divide_proof
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == obligation)
        .expect("wrapping-divide certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &mut corrupt.route
    else {
        panic!("wrapping divide must use a checked certificate")
    };
    let psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint, .. } =
        &mut certificate.proof.rule
    else {
        panic!("known divisor uses literal equality substitution")
    };
    *endpoint = 0;
    assert!(matches!(
        verify_module(
            &module,
            &corrupt_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            ..
        }) if rejected == obligation
    ));

    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(505), argument(0)],
    )
    .expect("verified wrapping division should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(101))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("wrapping division should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerDivide { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("wrapping division should select");
        let assigned =
            assign_registers(&target_operations).expect("wrapping-divide homes should assign");
        emit_machine_code(&assigned).expect("wrapping division should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("wrapping-divide host selection");
        let assigned = assign_registers(&target_operations).expect("wrapping-divide host homes");
        let machine_code = emit_machine_code(&assigned).expect("wrapping-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("wrapping-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 505, 0, 101));
    }
}

#[test]
fn only_canonical_nonzero_rows_bypass_legacy_divisor_reduction() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor source canaries should compile");
    for machine in [
        "terminal_exact_divide_known_right",
        "terminal_exact_remainder_known_right",
    ] {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        let obligation = lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ExactIntegerDivide { obligation, .. }
                | OperationKind::ExactIntegerRemainder { obligation, .. } => Some(obligation),
                _ => None,
            })
            .expect("controlled operation owns an obligation");
        let reconstructed =
            psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("{machine} should reconstruct: {error:?}"));
        assert_eq!(
            reconstructed
                .iter()
                .find(|site| site.obligation.id == obligation)
                .expect("operation obligation is reconstructed")
                .obligation
                .proposition,
            psi_core::Proposition::Truth,
            "{machine} remains on its literal-aware legacy reducer"
        );
    }
}

#[test]
fn checked_source_signed_wrapping_divide_wraps_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed wrapping-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_wrapping_divide_min")
        .expect("known signed wrapping division should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed wrapping-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed wrapping-divide proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed wrapping division should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(i64::MIN as i128))
    );

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed wrapping division should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed wrapping-divide host selection");
        let assigned = assign_registers(&target_operations).expect("signed wrapping-divide homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed wrapping-divide host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed wrapping-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            i64::MIN as u64,
        ));
    }
}

#[test]
fn checked_source_wrapping_remainder_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor wrapping-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_remainder_known_right")
        .expect("known nonzero wrapping remainder should lower");
    let remainder_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::WrappingIntegerRemainder { .. }
            )
        })
        .expect("proof-gated wrapping remainder remains explicit terminal work");
    let OperationKind::WrappingIntegerRemainder { obligation, .. } = remainder_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&remainder_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("wrapping-remainder obligation reconstructs");
    let site = reconstructed
        .iter()
        .find(|site| site.obligation.id == obligation)
        .expect("wrapping-remainder obligation is reconstructed");
    let OperationKind::WrappingIntegerRemainder { right, .. } = remainder_operation.kind else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    assert_eq!(
        site.obligation.proposition,
        psi_core::Proposition::LessOrEqual(
            psi_core::ScalarTerm::integer(u32_type, IntegerValue::Unsigned(1)).unwrap(),
            psi_core::ScalarTerm::value(right, ScalarType::Integer(u32_type)),
        )
    );
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .expect("wrapping-remainder certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route
    else {
        panic!("wrapping remainder must use a checked certificate")
    };
    assert!(matches!(
        certificate.proof.rule,
        psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
    ));

    let semantic = encode_module(&lowered.semantic_module).expect("wrapping-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("wrapping-remainder proof");
    let module = decode_module(&semantic).expect("decode wrapping-remainder semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_remainder_proof =
        decode_proof_bundle(&proof).expect("decode wrapping-remainder proof");
    missing_remainder_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let mut corrupt_remainder_proof =
        decode_proof_bundle(&proof).expect("decode wrapping-remainder proof");
    let corrupt = corrupt_remainder_proof
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == obligation)
        .expect("wrapping-remainder certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &mut corrupt.route
    else {
        panic!("wrapping remainder must use a checked certificate")
    };
    let psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint, .. } =
        &mut certificate.proof.rule
    else {
        panic!("known divisor uses literal equality substitution")
    };
    *endpoint = 0;
    assert!(matches!(
        verify_module(
            &module,
            &corrupt_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            ..
        }) if rejected == obligation
    ));

    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(503), argument(0)],
    )
    .expect("verified wrapping remainder should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(3))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("wrapping remainder should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::WrappingIntegerRemainder { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("wrapping remainder should select");
        let assigned =
            assign_registers(&target_operations).expect("wrapping-remainder homes should assign");
        emit_machine_code(&assigned).expect("wrapping remainder should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("wrapping-remainder host selection");
        let assigned = assign_registers(&target_operations).expect("wrapping-remainder host homes");
        let machine_code = emit_machine_code(&assigned).expect("wrapping-remainder host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("wrapping-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 503, 0, 3));
    }
}

#[test]
fn checked_source_signed_wrapping_remainder_returns_zero_for_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed wrapping-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_wrapping_remainder_min")
        .expect("known signed wrapping remainder should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed wrapping-remainder semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("signed wrapping-remainder proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed wrapping remainder should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed wrapping remainder should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed wrapping-remainder host selection");
        let assigned =
            assign_registers(&target_operations).expect("signed wrapping-remainder homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed wrapping-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed wrapping-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            0,
        ));
    }
}

#[test]
fn checked_source_saturating_divide_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor saturating-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_saturating_divide_known_right")
        .expect("known nonzero saturating division should lower");
    let divide_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::SaturatingIntegerDivide { .. }
            )
        })
        .expect("proof-gated saturating division remains explicit terminal work");
    let OperationKind::SaturatingIntegerDivide { obligation, .. } = divide_operation.kind else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&divide_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("saturating-divide obligation reconstructs");
    let site = reconstructed
        .iter()
        .find(|site| site.obligation.id == obligation)
        .expect("saturating-divide obligation is reconstructed");
    let OperationKind::SaturatingIntegerDivide { right, .. } = divide_operation.kind else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    assert_eq!(
        site.obligation.proposition,
        psi_core::Proposition::LessOrEqual(
            psi_core::ScalarTerm::integer(u32_type, IntegerValue::Unsigned(1)).unwrap(),
            psi_core::ScalarTerm::value(right, ScalarType::Integer(u32_type)),
        )
    );
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .expect("saturating-divide certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route
    else {
        panic!("saturating divide must use a checked certificate")
    };
    assert!(matches!(
        certificate.proof.rule,
        psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
    ));

    let semantic = encode_module(&lowered.semantic_module).expect("saturating-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("saturating-divide proof");
    let module = decode_module(&semantic).expect("decode saturating-divide semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_divide_proof =
        decode_proof_bundle(&proof).expect("decode saturating-divide proof");
    missing_divide_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let mut corrupt_divide_proof =
        decode_proof_bundle(&proof).expect("decode saturating-divide proof");
    let corrupt = corrupt_divide_proof
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == obligation)
        .expect("saturating-divide certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &mut corrupt.route
    else {
        panic!("saturating divide must use a checked certificate")
    };
    let psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint, .. } =
        &mut certificate.proof.rule
    else {
        panic!("known divisor uses literal equality substitution")
    };
    *endpoint = 0;
    assert!(matches!(
        verify_module(
            &module,
            &corrupt_divide_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            ..
        }) if rejected == obligation
    ));

    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(505), argument(0)],
    )
    .expect("verified saturating division should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(101))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("saturating division should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::SaturatingIntegerDivide { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("saturating division should select");
        let assigned =
            assign_registers(&target_operations).expect("saturating-divide homes should assign");
        emit_machine_code(&assigned).expect("saturating division should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("saturating-divide host selection");
        let assigned = assign_registers(&target_operations).expect("saturating-divide host homes");
        let machine_code = emit_machine_code(&assigned).expect("saturating-divide host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("saturating-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 505, 0, 101));
    }
}

#[test]
fn checked_source_signed_saturating_divide_clamps_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed saturating-divide source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_saturating_divide_min")
        .expect("known signed saturating division should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed saturating-divide semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("signed saturating-divide proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed saturating division should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(i64::MAX as i128))
    );

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed saturating division should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed saturating-divide host selection");
        let assigned =
            assign_registers(&target_operations).expect("signed saturating-divide homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed saturating-divide host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed saturating-divide host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            i64::MAX as u64,
        ));
    }
}

#[test]
fn checked_source_saturating_remainder_uses_known_nonzero_divisor() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-divisor saturating-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_saturating_remainder_known_right")
        .expect("known nonzero saturating remainder should lower");
    let remainder_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::SaturatingIntegerRemainder { .. }
            )
        })
        .expect("proof-gated saturating remainder remains explicit terminal work");
    let OperationKind::SaturatingIntegerRemainder { obligation, .. } = remainder_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&remainder_operation.kind),
        1
    );
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("saturating-remainder obligation reconstructs");
    let site = reconstructed
        .iter()
        .find(|site| site.obligation.id == obligation)
        .expect("saturating-remainder obligation is reconstructed");
    let OperationKind::SaturatingIntegerRemainder { right, .. } = remainder_operation.kind else {
        unreachable!()
    };
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    assert_eq!(
        site.obligation.proposition,
        psi_core::Proposition::LessOrEqual(
            psi_core::ScalarTerm::integer(u32_type, IntegerValue::Unsigned(1)).unwrap(),
            psi_core::ScalarTerm::value(right, ScalarType::Integer(u32_type)),
        )
    );
    let evidence = lowered
        .proof_bundle
        .evidence
        .iter()
        .find(|evidence| evidence.obligation == obligation)
        .expect("saturating-remainder certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route
    else {
        panic!("saturating remainder must use a checked certificate")
    };
    assert!(matches!(
        certificate.proof.rule,
        psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
    ));

    let semantic = encode_module(&lowered.semantic_module).expect("saturating-remainder semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("saturating-remainder proof");
    let module = decode_module(&semantic).expect("decode saturating-remainder semantics");
    assert_eq!(module.vocabulary_marker, VocabularyMarker::CURRENT);
    let mut missing_remainder_proof =
        decode_proof_bundle(&proof).expect("decode saturating-remainder proof");
    missing_remainder_proof
        .evidence
        .retain(|evidence| evidence.obligation != obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(missing))
            if missing == obligation
    ));

    let mut corrupt_remainder_proof =
        decode_proof_bundle(&proof).expect("decode saturating-remainder proof");
    let corrupt = corrupt_remainder_proof
        .evidence
        .iter_mut()
        .find(|evidence| evidence.obligation == obligation)
        .expect("saturating-remainder certificate exists");
    let psi_proof_admission::EvidenceRoute::CertificateDerived(certificate) = &mut corrupt.route
    else {
        panic!("saturating remainder must use a checked certificate")
    };
    let psi_proof_admission::ProofRule::IntegerLessOrEqualSubstitution { endpoint, .. } =
        &mut certificate.proof.rule
    else {
        panic!("known divisor uses literal equality substitution")
    };
    *endpoint = 0;
    assert!(matches!(
        verify_module(
            &module,
            &corrupt_remainder_proof,
            &AdmissionProfile::default()
        ),
        Err(psi_terminal_verifier::VerificationError::RejectedEvidence {
            obligation: rejected,
            ..
        }) if rejected == obligation
    ));

    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(507), argument(0)],
    )
    .expect("verified saturating remainder should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(2))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("saturating remainder should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::SaturatingIntegerRemainder { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("saturating remainder should select");
        let assigned =
            assign_registers(&target_operations).expect("saturating-remainder homes should assign");
        emit_machine_code(&assigned).expect("saturating remainder should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("saturating-remainder host selection");
        let assigned =
            assign_registers(&target_operations).expect("saturating-remainder host homes");
        let machine_code =
            emit_machine_code(&assigned).expect("saturating-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("saturating-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(entry, 507, 0, 2));
    }
}

#[test]
fn checked_source_signed_saturating_remainder_returns_zero_for_minimum_by_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("signed saturating-remainder source canary should compile");
    let lowered = lower_machine(&checked, "terminal_signed_saturating_remainder_min")
        .expect("known signed saturating remainder should lower");
    let semantic =
        encode_module(&lowered.semantic_module).expect("signed saturating-remainder semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("signed saturating-remainder proof");
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i64_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(i64::MIN as i128), argument(0)],
    )
    .expect("verified signed saturating remainder should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    #[cfg(unix)]
    {
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .expect("signed saturating remainder should cross the Omega boundary");
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("signed saturating-remainder host selection");
        let assigned =
            assign_registers(&target_operations).expect("signed saturating-remainder homes");
        let machine_code =
            emit_machine_code(&assigned).expect("signed saturating-remainder host emission");
        let object = build_terminal_object_artifact(&machine_code)
            .expect("signed saturating-remainder host object");
        let entry = object.entry_function().bytes(&object);
        assert!(host_machine_code_with_two_u64_matches(
            entry,
            i64::MIN as u64,
            0,
            0,
        ));
    }
}

#[test]
fn checked_source_guarded_runtime_divisors_cross_every_fixed_integer_policy() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("guarded runtime-divisor source canaries should compile");
    let cases = [
        ("terminal_exact_divide_guarded_right", 4_u128),
        ("terminal_exact_remainder_guarded_right", 3_u128),
        ("terminal_wrapping_divide_guarded_right", 4_u128),
        ("terminal_wrapping_remainder_guarded_right", 3_u128),
        ("terminal_saturating_divide_guarded_right", 4_u128),
        ("terminal_saturating_remainder_guarded_right", 3_u128),
    ];
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };

    for (machine, expected) in cases {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let obligation = lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ExactIntegerDivide { obligation, .. }
                | OperationKind::ExactIntegerRemainder { obligation, .. }
                | OperationKind::WrappingIntegerDivide { obligation, .. }
                | OperationKind::WrappingIntegerRemainder { obligation, .. }
                | OperationKind::SaturatingIntegerDivide { obligation, .. }
                | OperationKind::SaturatingIntegerRemainder { obligation, .. } => Some(obligation),
                _ => None,
            })
            .expect("guarded operation owns a divisor obligation");
        assert!(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .any(|evidence| evidence.obligation == obligation)
        );

        let semantic = encode_module(&lowered.semantic_module).expect("guarded semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("guarded proof");
        verify_module(
            &decode_module(&semantic).expect("decode guarded semantics"),
            &decode_proof_bundle(&proof).expect("decode guarded proof"),
            &AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{machine} artifact should verify: {error:?}"));
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(23), argument(5)],
        )
        .unwrap_or_else(|error| panic!("{machine} should interpret: {error:?}"));
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(argument(expected))
        );
        let zero_path = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(23), argument(0)],
        )
        .unwrap_or_else(|error| panic!("{machine} zero path should bypass arithmetic: {error:?}"));
        assert_eq!(
            zero_path.value(),
            TerminalExecutionResult::Scalar(argument(0))
        );

        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }
    }
}

#[test]
fn checked_source_guarded_negative_runtime_divisor_excludes_zero_and_negative_one() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("guarded negative runtime-divisor source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_divide_guarded_negative_right")
        .expect("divisor <= -2 should lower exact signed division");
    assert_eq!(
        lowered.semantic_module.vocabulary_marker,
        VocabularyMarker::CURRENT
    );
    let semantic = encode_module(&lowered.semantic_module).expect("negative-divisor semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("negative-divisor proof");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    let execution = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(23), argument(-5)],
    )
    .expect("negative guarded divisor should interpret");
    assert_eq!(
        execution.value(),
        TerminalExecutionResult::Scalar(argument(-4))
    );
    let bypassed = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[argument(23), argument(-1)],
    )
    .expect("negative one should take the bypass arm");
    assert_eq!(
        bypassed.value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("negative-divisor artifact should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("negative-divisor control should select");
        let assigned =
            assign_registers(&target_operations).expect("negative-divisor control should assign");
        emit_machine_code(&assigned).expect("negative-divisor control should emit");
    }
}

#[test]
fn checked_source_negative_one_range_uses_policy_appropriate_dividend_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("negative-one-range source canaries should compile");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: i32_type,
        value: IntegerValue::Signed(value),
    };
    for (machine, value, expected) in [
        (
            "terminal_exact_divide_guarded_negative_one_range",
            23_i128,
            -23_i128,
        ),
        (
            "terminal_wrapping_divide_guarded_negative_one_range",
            i32::MIN as i128,
            i32::MIN as i128,
        ),
    ] {
        let lowered = lower_machine(&checked, machine)
            .unwrap_or_else(|error| panic!("{machine} should lower: {error:?}"));
        assert_eq!(
            lowered.semantic_module.vocabulary_marker,
            VocabularyMarker::CURRENT
        );
        let semantic = encode_module(&lowered.semantic_module).expect("range semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("range proof");
        let execution = interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(-1)],
        )
        .unwrap_or_else(|error| panic!("{machine} should interpret: {error:?}"));
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(argument(expected))
        );
        let abstract_operations =
            lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
                .unwrap_or_else(|error| panic!("{machine} should cross Omega: {error:?}"));
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target_operations = lower_to_target_operations(&abstract_operations, target)
                .unwrap_or_else(|error| panic!("{machine} should select: {error:?}"));
            let assigned = assign_registers(&target_operations)
                .unwrap_or_else(|error| panic!("{machine} should assign: {error:?}"));
            emit_machine_code(&assigned)
                .unwrap_or_else(|error| panic!("{machine} should emit: {error:?}"));
        }
    }
}
