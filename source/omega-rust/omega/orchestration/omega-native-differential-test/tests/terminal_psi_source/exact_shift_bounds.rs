use super::*;
use omega_optimization_core::{
    AcceptedObligationFactIdentity, AnalysisKind, OptimizationUnitIdentity,
};
use omega_optimization_unit::{
    ValueRangeFact, ValueRangeScope, ValueRangeSupport, value_range_fact_identity,
};
use omega_optimization_validation::{
    OptimizationUnitValidationError, validate_current_value_range_fact,
    validate_current_value_range_fact_at,
};
use omega_psi_optimizer::{AnalysisProduct, compute_analysis};

#[test]
fn checked_source_guarded_exact_narrowing_carries_independently_verified_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("guarded exact-narrowing source canary should compile");
    let lowered = lower_machine(&checked, "terminal_guarded_exact_narrow")
        .expect("guarded exact narrowing should lower with path evidence");
    let cast_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::IntegerExactCast { .. }))
        .expect("the runtime narrowing remains explicit terminal work");
    let OperationKind::IntegerExactCast {
        obligation: cast_obligation,
        ..
    } = cast_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&cast_operation.kind),
        1
    );
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == cast_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));

    let semantic = encode_module(&lowered.semantic_module).expect("guarded narrowing semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("guarded narrowing proof");
    let module = decode_module(&semantic).expect("decode guarded narrowing semantics");
    let mut missing_cast_proof = decode_proof_bundle(&proof).expect("decode guarded proof");
    missing_cast_proof
        .evidence
        .retain(|evidence| evidence.obligation != cast_obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_cast_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == cast_obligation
    ));

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(0)],
        )
        .expect("verified guarded narrowing should interpret")
    };
    let narrowed = execute(255);
    let rejected = execute(256);
    assert_eq!(
        narrowed.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(255),
        })
    );
    assert_eq!(
        rejected.value(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: u8_type,
            value: IntegerValue::Unsigned(0),
        })
    );
    assert_eq!(
        narrowed.usage().total_units(),
        rejected.usage().total_units()
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("guarded narrowing should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::IntegerExactCast { .. }
                )
            })
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("guarded narrowing should select");
        let assigned =
            assign_registers(&target_operations).expect("guarded narrowing homes should assign");
        emit_machine_code(&assigned).expect("guarded narrowing should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("guarded narrowing host selection");
        let assigned = assign_registers(&target_operations).expect("guarded narrowing host homes");
        let machine_code = emit_machine_code(&assigned).expect("guarded narrowing host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("guarded narrowing host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 255, 0), 255);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 256, 0), 0);
    }
}

#[test]
fn checked_source_exact_right_shift_carries_independently_verified_count_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("exact right-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_right_runtime")
        .expect("exact right shift should lower with path evidence");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftRight { .. }))
        .expect("the proof-gated right shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftRight {
        count,
        obligation: shift_obligation,
        ..
    } = shift_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&shift_operation.kind),
        1
    );
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == shift_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));
    let reconstructed =
        psi_terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module)
            .expect("exact right-shift obligation reconstructs");
    let site = reconstructed
        .iter()
        .find(|site| site.obligation.id == shift_obligation)
        .expect("exact right-shift obligation is reconstructed");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    assert!(
        site.canonical_certificate,
        "right-shift count custody must select the unchanged canonical goal",
    );
    assert_eq!(
        site.obligation.proposition,
        psi_core::Proposition::LessOrEqual(
            psi_core::ScalarTerm::value(count, ScalarType::Integer(u64_type)),
            psi_core::ScalarTerm::integer(u64_type, IntegerValue::Unsigned(63)).unwrap(),
        ),
    );

    let semantic = encode_module(&lowered.semantic_module).expect("exact shift semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact shift proof");
    let module = decode_module(&semantic).expect("decode exact shift semantics");
    let mut missing_shift_proof = decode_proof_bundle(&proof).expect("decode exact shift proof");
    missing_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != shift_obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == shift_obligation
    ));

    let optimizer_input =
        omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
        )
        .expect("exact shift verifies for optimizer admission");
    let verified = omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        optimizer_input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("exact shift retains proof custody in the optimization unit");
    let AnalysisProduct::ValueRanges(ranges) =
        compute_analysis(verified.unit(), AnalysisKind::ValueRanges).unwrap()
    else {
        unreachable!()
    };
    let range = ranges
        .facts
        .iter()
        .find(|fact| {
            fact.value == count
                && matches!(
                    fact.support,
                    ValueRangeSupport::AcceptedOperationProof {
                        operation,
                        ..
                    } if operation == shift_operation.id
                )
        })
        .expect("accepted exact-shift proof derives a current count interval");
    assert_eq!(range.scalar_type, u64_type);
    assert_eq!(range.minimum, IntegerValue::Unsigned(0));
    assert_eq!(range.maximum, IntegerValue::Unsigned(63));
    validate_current_value_range_fact(verified.unit(), range)
        .expect("independent validation reconstructs the proof-derived interval");
    let ValueRangeScope::DominatedOperationEntry {
        block: owner_block,
        node: owner_node,
        operation,
    } = range.valid_in.scope
    else {
        panic!("proof-derived range has an operation-entry scope")
    };
    assert_eq!(operation, shift_operation.id);
    validate_current_value_range_fact_at(
        verified.unit(),
        range,
        range.valid_in.machine,
        owner_block,
        owner_node,
    )
    .expect("the proof-derived interval applies at its operation entry");
    assert!(ranges.fact_applies_at(
        range,
        verified.unit(),
        range.valid_in.machine,
        owner_block,
        owner_node,
    ));
    if owner_node > 0 {
        assert!(matches!(
            validate_current_value_range_fact_at(
                verified.unit(),
                range,
                range.valid_in.machine,
                owner_block,
                owner_node - 1,
            ),
            Err(OptimizationUnitValidationError::CurrentValueRangeFactNotApplicable { .. })
        ));
        assert!(!ranges.fact_applies_at(
            range,
            verified.unit(),
            range.valid_in.machine,
            owner_block,
            owner_node - 1,
        ));
    }
    let function = verified
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == range.valid_in.machine)
        .expect("range owner machine remains current");
    for block in &function.blocks {
        if block.id == owner_block || block.nodes.is_empty() {
            continue;
        }
        assert_eq!(
            ranges.fact_applies_at(range, verified.unit(), function.machine, block.id, 0,),
            range
                .valid_in
                .dominated_blocks
                .binary_search(&block.id)
                .is_ok(),
            "range applicability follows its exact current dominated-block roster"
        );
        let independently_applies = validate_current_value_range_fact_at(
            verified.unit(),
            range,
            function.machine,
            block.id,
            0,
        )
        .is_ok();
        assert_eq!(
            independently_applies,
            range
                .valid_in
                .dominated_blocks
                .binary_search(&block.id)
                .is_ok(),
            "independent validation reconstructs the same current dominance region"
        );
    }

    let refresh_range_identity = |fact: &mut ValueRangeFact| {
        fact.identity = value_range_fact_identity(
            fact.value,
            fact.scalar_type,
            fact.minimum,
            fact.maximum,
            &fact.support,
            &fact.valid_in,
        )
        .expect("self-consistent corruption remains structurally encodable");
    };
    let rejects = |fact: &ValueRangeFact, axis| {
        assert_eq!(
            validate_current_value_range_fact(verified.unit(), fact),
            Err(OptimizationUnitValidationError::CurrentValueRangeFactMismatch),
            "independent reconstruction rejects self-consistent {axis} corruption"
        );
    };

    let mut corrupted = range.clone();
    corrupted.maximum = IntegerValue::Unsigned(62);
    refresh_range_identity(&mut corrupted);
    rejects(&corrupted, "bound");

    let mut corrupted = range.clone();
    corrupted.scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    refresh_range_identity(&mut corrupted);
    rejects(&corrupted, "type");

    let mut corrupted = range.clone();
    corrupted.valid_in.revision =
        OptimizationUnitIdentity::from_canonical_bytes(b"stale range revision");
    refresh_range_identity(&mut corrupted);
    rejects(&corrupted, "revision");

    let mut corrupted = range.clone();
    let ValueRangeSupport::AcceptedOperationProof { accepted, .. } = &mut corrupted.support else {
        unreachable!()
    };
    *accepted = AcceptedObligationFactIdentity::from_canonical_bytes(b"forged range support");
    refresh_range_identity(&mut corrupted);
    rejects(&corrupted, "support");

    let mut corrupted = range.clone();
    let ValueRangeScope::DominatedOperationEntry { node, .. } = &mut corrupted.valid_in.scope
    else {
        unreachable!()
    };
    *node = node.saturating_add(1);
    refresh_range_identity(&mut corrupted);
    rejects(&corrupted, "anchor");

    if let Some(position) = range
        .valid_in
        .dominated_blocks
        .iter()
        .position(|block| *block != owner_block)
    {
        let mut corrupted = range.clone();
        corrupted.valid_in.dominated_blocks.remove(position);
        refresh_range_identity(&mut corrupted);
        rejects(&corrupted, "dominance-roster");
    }

    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified exact right shift should interpret")
    };
    let shifted = execute(1u128 << 63, 63);
    let rejected = execute(1u128 << 63, 64);
    assert_eq!(
        shifted.value(),
        TerminalExecutionResult::Scalar(argument(1))
    );
    assert_eq!(
        rejected.value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(shifted.usage().total_units(), 6);
    assert_eq!(
        shifted.usage().total_units(),
        rejected.usage().total_units()
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact shift should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| {
                matches!(
                    operation,
                    TerminalAbstractOperation::ExactIntegerShiftRight { .. }
                )
            })
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact shift should select");
        let assigned =
            assign_registers(&target_operations).expect("exact shift homes should assign");
        emit_machine_code(&assigned).expect("exact shift should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact shift host selection");
        let assigned = assign_registers(&target_operations).expect("exact shift host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact shift host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact shift host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1u64 << 63, 63), 1);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1u64 << 63, 64), 0);
    }
}

#[test]
fn checked_source_exact_left_shift_carries_count_and_value_evidence() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_runtime")
        .expect("exact left shift should lower with path evidence");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. }))
        .expect("the proof-gated left shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftLeft {
        obligation: shift_obligation,
        ..
    } = shift_operation.kind
    else {
        unreachable!()
    };
    assert_eq!(
        TerminalFuelSchedule::CURRENT.operation_units(&shift_operation.kind),
        1
    );
    assert!(lowered.proof_bundle.evidence.iter().any(|evidence| {
        evidence.obligation == shift_obligation
            && matches!(
                evidence.route,
                psi_proof_admission::EvidenceRoute::CertificateDerived(_)
            )
    }));

    let semantic = encode_module(&lowered.semantic_module).expect("exact left-shift semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("exact left-shift proof");
    let module = decode_module(&semantic).expect("decode exact left-shift semantics");
    let mut missing_shift_proof =
        decode_proof_bundle(&proof).expect("decode exact left-shift proof");
    missing_shift_proof
        .evidence
        .retain(|evidence| evidence.obligation != shift_obligation);
    assert!(matches!(
        verify_module(
            &module,
            &missing_shift_proof,
            &AdmissionProfile::default(),
        ),
        Err(psi_terminal_verifier::VerificationError::MissingEvidence(obligation))
            if obligation == shift_obligation
    ));

    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified exact left shift should interpret")
    };
    let shifted = execute(1, 31);
    let rejected_value = execute(2, 31);
    let rejected_count = execute(1, 32);
    assert_eq!(
        shifted.value(),
        TerminalExecutionResult::Scalar(argument(1u128 << 31))
    );
    assert_eq!(
        rejected_value.value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        rejected_count.value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        shifted.usage().total_units(),
        rejected_value.usage().total_units()
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("exact left shift should cross the Omega boundary");
    assert!(
        abstract_operations.functions[0]
            .operations
            .iter()
            .any(|operation| matches!(
                operation,
                TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
            ))
    );
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("exact left shift should select");
        let assigned =
            assign_registers(&target_operations).expect("exact left-shift homes should assign");
        emit_machine_code(&assigned).expect("exact left shift should emit");
    }

    #[cfg(unix)]
    {
        let target_operations =
            lower_to_target_operations(&abstract_operations, NativeTarget::host())
                .expect("exact left-shift host selection");
        let assigned = assign_registers(&target_operations).expect("exact left-shift host homes");
        let machine_code = emit_machine_code(&assigned).expect("exact left-shift host emission");
        let object =
            build_terminal_object_artifact(&machine_code).expect("exact left-shift host object");
        let entry = object.entry_function().bytes(&object);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1, 5), 32);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 2, 31), 0);
        assert_eq!(run_host_machine_code_with_two_u64(entry, 1, 32), 0);
    }
}

#[test]
fn checked_source_exact_left_shift_uses_known_count_bounds() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("known-count exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_known_count")
        .expect("known-count exact left shift should use the precise value bound");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. }))
        .expect("known-count exact left shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftLeft { obligation, .. } = shift_operation.kind else {
        unreachable!()
    };
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic =
        encode_module(&lowered.semantic_module).expect("known-count exact left-shift semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("known-count exact left-shift proof");
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
            &[argument(value)],
        )
        .expect("verified known-count exact left shift should interpret")
    };
    assert_eq!(
        execute(536_870_911).value(),
        TerminalExecutionResult::Scalar(argument(4_294_967_288))
    );
    assert_eq!(
        execute(536_870_912).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("known-count exact left shift should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("known-count exact left shift should select");
        let assigned =
            assign_registers(&target_operations).expect("known-count exact left-shift homes");
        emit_machine_code(&assigned).expect("known-count exact left shift should emit");
    }
}

#[test]
fn checked_source_exact_left_shift_uses_bounded_count_maximum() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("bounded-count exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_bounded_count")
        .expect("bounded-count exact left shift should use its proved maximum count");
    let shift_operation = lowered.semantic_module.machines[0]
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::ExactIntegerShiftLeft { .. }))
        .expect("bounded-count exact left shift remains explicit terminal work");
    let OperationKind::ExactIntegerShiftLeft { obligation, .. } = shift_operation.kind else {
        unreachable!()
    };
    assert!(
        lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == obligation)
    );

    let semantic =
        encode_module(&lowered.semantic_module).expect("bounded-count exact left-shift semantics");
    let proof =
        encode_proof_bundle(&lowered.proof_bundle).expect("bounded-count exact left-shift proof");
    let u32_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u32_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified bounded-count exact left shift should interpret")
    };
    assert_eq!(
        execute(536_870_911, 3).value(),
        TerminalExecutionResult::Scalar(argument(4_294_967_288))
    );
    assert_eq!(
        execute(536_870_911, 2).value(),
        TerminalExecutionResult::Scalar(argument(2_147_483_644))
    );
    assert_eq!(
        execute(536_870_912, 3).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(1, 4).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("bounded-count exact left shift should cross the Omega boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("bounded-count exact left shift should select");
        let assigned =
            assign_registers(&target_operations).expect("bounded-count exact left-shift homes");
        emit_machine_code(&assigned).expect("bounded-count exact left shift should emit");
    }
}

#[test]
fn checked_source_exact_left_shift_uses_u64_bounded_count_maximum() {
    let checked = compile_to_checked(&source_canary(), None)
        .expect("u64 bounded-count exact left-shift source canary should compile");
    let lowered = lower_machine(&checked, "terminal_exact_shift_left_u64_bounded_count")
        .expect("u64 bounded-count exact left shift should use its value and count bounds");
    let semantic =
        encode_module(&lowered.semantic_module).expect("u64 bounded-count shift semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("u64 bounded-count shift proof");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let argument = |value| TerminalScalarValue::Integer {
        scalar_type: u64_type,
        value: IntegerValue::Unsigned(value),
    };
    let execute = |value, count| {
        interpret_terminal_artifact_measured(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[argument(value), argument(count)],
        )
        .expect("verified u64 bounded-count exact left shift should interpret")
    };
    assert_eq!(
        execute(2_305_843_009_213_693_951, 3).value(),
        TerminalExecutionResult::Scalar(argument(u64::MAX as u128 - 7))
    );
    assert_eq!(
        execute(2_305_843_009_213_693_951, 2).value(),
        TerminalExecutionResult::Scalar(argument(9_223_372_036_854_775_804))
    );
    assert_eq!(
        execute(2_305_843_009_213_693_952, 3).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );
    assert_eq!(
        execute(1, 4).value(),
        TerminalExecutionResult::Scalar(argument(0))
    );

    let abstract_operations =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
            .expect("u64 bounded-count exact left shift should cross Omega");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_operations = lower_to_target_operations(&abstract_operations, target)
            .expect("u64 bounded-count exact left shift should select");
        let assigned = assign_registers(&target_operations)
            .expect("u64 bounded-count exact left-shift homes should assign");
        emit_machine_code(&assigned).expect("u64 bounded-count exact left shift should emit");
    }
}
