use super::{integer_signature, strict_x86_entry};
use crate::plans::{
    BoundaryPlanDiagnostic, BoundaryPlanResult, CallSignature, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, MachineRegime, MachineRegister, MachineState,
    MachineStateSet, Preemption, ProviderExitRealization, RegisterSet, StateFootprintEvidence,
    StatePlan, SystemVEightbyteClass, ValueShape, encode_state_plan_identity,
    evaluate_freestanding_program_entry_plan, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan, validate_boundary_plan_result, validate_composed_state_footprint,
    validate_provider_exit_realization, validate_runtime_value_guard_footprint,
    validate_state_footprint,
};

#[test]
fn ordinary_firmware_entry_has_no_interrupted_state_obligation() {
    let signature = integer_signature(2);
    let validated = evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("ordinary Microsoft x64 boundary entry");
    let plan = validated.plan();

    assert_eq!(plan.state.initial_regime, MachineRegime::X86Long64);
    assert!(plan.state.interrupted_state.is_empty());
    assert!(plan.state.saved_state.is_empty());
    assert!(plan.state.restored_state.is_empty());
    assert_eq!(plan.state.stack, EntryStack::ProviderSelected);
    assert_eq!(plan.state.preemption, Preemption::NotApplicable);
    assert!(
        plan.state
            .permitted_transitive_use
            .contains_all(MachineStateSet::new([MachineState::GeneralRegisters]))
    );
    assert!(
        plan.state
            .permitted_transitive_use
            .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
    );
    assert!(
        plan.state
            .permitted_transitive_use
            .contains_all(MachineStateSet::new([MachineState::Flags]))
    );
    validate_state_footprint(
        &validated,
        &StateFootprintEvidence::new(
            RegisterSet::default(),
            MachineStateSet::new([MachineState::Flags]),
        ),
    )
    .expect("ordinary caller-volatile condition flags fit the state ceiling");
}

#[test]
fn implicit_freestanding_entry_admits_boot_root_machine_state() {
    let signature = integer_signature(1);
    let hosted = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary entry");
    let freestanding =
        evaluate_freestanding_program_entry_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("implicit freestanding entry");
    let boot_root_state = MachineStateSet::new([
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::ControlState,
    ]);

    assert!(
        !hosted
            .plan()
            .state
            .permitted_transitive_use
            .contains_all(boot_root_state)
    );
    assert!(
        freestanding
            .plan()
            .state
            .permitted_transitive_use
            .contains_all(boot_root_state)
    );
    assert_eq!(
        hosted.plan().state.interrupted_state,
        freestanding.plan().state.interrupted_state
    );
    assert_eq!(
        hosted.plan().state.saved_state,
        freestanding.plan().state.saved_state
    );
    assert_eq!(
        hosted.plan().state.restored_state,
        freestanding.plan().state.restored_state
    );
}

#[test]
fn provider_exit_realization_must_match_the_complete_boundary_exit() {
    let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
        .expect("strict interrupt boundary");
    let expected = ProviderExitRealization {
        control: validated.plan().call.entry_control,
        restored_state: validated.plan().state.restored_state,
    };
    validate_provider_exit_realization(validated.plan(), &expected)
        .expect("exact provider exit realization");

    for (realization, expected_diagnostic) in [
        (
            ProviderExitRealization {
                control: EntryControl::CallReturn,
                ..expected
            },
            "exit control",
        ),
        (
            ProviderExitRealization {
                restored_state: MachineStateSet::empty(),
                ..expected
            },
            "restored-state set",
        ),
    ] {
        let error = validate_provider_exit_realization(validated.plan(), &realization)
            .expect_err("drifted provider exit must reject");
        assert!(
            error.0.contains(expected_diagnostic),
            "expected `{expected_diagnostic}`, got `{error}`"
        );
    }
}

#[test]
fn runtime_value_guard_stack_scratch_is_not_admitted_for_interrupt_return() {
    let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
        .expect("strict interrupt boundary");
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86R10]),
        MachineStateSet::new([MachineState::Flags, MachineState::StackPointer]),
    );

    let error = validate_runtime_value_guard_footprint(&validated, &evidence)
        .expect_err("interrupt-return body must not borrow ordinary stack scratch");

    assert!(error.0.contains("x86 call-return activation"));
}

#[test]
fn runtime_value_guard_control_state_is_not_admitted_for_interrupt_return() {
    let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
        .expect("strict interrupt boundary");
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86R10]),
        MachineStateSet::new([MachineState::Flags, MachineState::ControlState]),
    );

    let error = validate_runtime_value_guard_footprint(&validated, &evidence)
        .expect_err("interrupt-return body must not change floating control state");

    assert!(
        error
            .0
            .contains("directed rounding requires a call-return activation")
    );
}

#[test]
fn boundary_entry_validation_rejects_unsaved_permitted_state() {
    let mut plan = strict_x86_entry();
    validate_boundary_entry_plan(plan.clone(), &integer_signature(1))
        .expect("strict plan is coherent");
    plan.state.permitted_transitive_use = plan
        .state
        .permitted_transitive_use
        .union(MachineStateSet::new([MachineState::VectorRegisters]));
    let error =
        validate_boundary_entry_plan(plan, &integer_signature(1)).expect_err("SIMD is not saved");
    assert!(error.0.contains("does not save"));
}

#[test]
fn evidence_is_validated_but_firewalled_from_contract_identity() {
    let plan = strict_x86_entry();
    let validated = validate_boundary_entry_plan(plan, &integer_signature(1)).expect("entry plan");
    let identity = validated.contract_report_fingerprint();
    let evidence_a = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax]),
        MachineStateSet::new([MachineState::GeneralRegisters]),
    );
    let evidence_b = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rcx]),
        MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags]),
    );
    validate_state_footprint(&validated, &evidence_a).expect("first footprint");
    validate_state_footprint(&validated, &evidence_b).expect("second footprint");
    assert_ne!(
        evidence_a.evidence_report_fingerprint(),
        evidence_b.evidence_report_fingerprint()
    );
    assert_eq!(identity, validated.contract_report_fingerprint());
}

#[test]
fn fragment_footprints_compose_deterministically_before_validation() {
    let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
        .expect("strict entry plan");
    let entry = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86R11, MachineRegister::X86Rax]),
        MachineStateSet::empty(),
    );
    let body = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rcx, MachineRegister::X86R11]),
        MachineStateSet::new([MachineState::Flags]),
    );

    let first = validate_composed_state_footprint(&validated, [&entry, &body, &entry])
        .expect("whole-entry footprint");
    let second = validate_composed_state_footprint(&validated, [&body, &entry])
        .expect("reordered whole-entry footprint");

    assert_eq!(first, second);
    assert_eq!(
        first.registers().as_slice(),
        &[
            MachineRegister::X86Rax,
            MachineRegister::X86Rcx,
            MachineRegister::X86R11,
        ]
    );
    assert_eq!(
        first.machine_state(),
        MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags])
    );
    assert_eq!(
        first.evidence_report_fingerprint(),
        second.evidence_report_fingerprint()
    );
}

#[test]
fn composed_footprint_rejects_one_fragment_above_the_state_ceiling() {
    let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
        .expect("strict entry plan");
    let entry = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax]),
        MachineStateSet::empty(),
    );
    let veneer = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Xmm(0)]),
        MachineStateSet::empty(),
    );

    let error = validate_composed_state_footprint(&validated, [&entry, &veneer])
        .expect_err("one vector-using fragment must reject the aggregate");

    assert!(error.0.contains("ceiling"));
}

#[test]
fn composed_footprint_rejects_a_foreign_architecture_fragment() {
    let validated = validate_boundary_entry_plan(strict_x86_entry(), &integer_signature(1))
        .expect("strict entry plan");
    let entry = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax]),
        MachineStateSet::empty(),
    );
    let foreign_thunk = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::Aarch64X(16)]),
        MachineStateSet::empty(),
    );

    let error = validate_composed_state_footprint(&validated, [&entry, &foreign_thunk])
        .expect_err("foreign-architecture evidence must reject the aggregate");

    assert!(error.0.contains("wrong architecture"));
}

#[test]
fn evaluated_state_plan_changes_contract_identity() {
    let first = strict_x86_entry();
    let mut second = first.clone();
    second.state.stack = EntryStack::Dedicated { class: 2 };
    let first =
        validate_boundary_entry_plan(first, &integer_signature(1)).expect("first entry plan");
    let second =
        validate_boundary_entry_plan(second, &integer_signature(1)).expect("second entry plan");
    assert_ne!(
        first.contract_report_fingerprint(),
        second.contract_report_fingerprint()
    );
}

#[test]
fn accepted_policy_results_canonicalize_fragment_order_before_identity() {
    let shape = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse,
    );
    let signature = CallSignature {
        parameters: vec![shape],
        result: None,
    };
    let baseline = evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("ordinary mixed-aggregate entry");
    let mut authored = baseline.plan().clone();
    authored.call.parameters[0].locations.reverse();

    let accepted =
        validate_boundary_plan_result(BoundaryPlanResult::Accepted(authored), &signature)
            .expect("semantically equivalent authored plan");

    assert_eq!(
        accepted.plan().call.parameters[0].locations,
        baseline.plan().call.parameters[0].locations
    );
    assert_eq!(
        accepted.contract_report_fingerprint(),
        baseline.contract_report_fingerprint()
    );
}

#[test]
fn rejected_policy_results_cannot_acquire_contract_identity() {
    let result = validate_boundary_plan_result(
        BoundaryPlanResult::Rejected(CallingPolicyRejection::new(
            "interrupt policies do not admit return values",
        )),
        &integer_signature(1),
    );

    let BoundaryPlanDiagnostic::Rejected(rejection) =
        result.expect_err("policy rejection must not validate")
    else {
        panic!("policy rejection was reported as a malformed accepted plan");
    };
    assert_eq!(
        rejection.reason(),
        "interrupt policies do not admit return values"
    );
}

#[test]
fn invalid_accepted_policy_plan_is_distinct_from_policy_rejection() {
    let mut plan = strict_x86_entry();
    plan.call.stack_alignment = 3;
    let result =
        validate_boundary_plan_result(BoundaryPlanResult::Accepted(plan), &integer_signature(1));

    let BoundaryPlanDiagnostic::InvalidAcceptedPlan(diagnostic) =
        result.expect_err("invalid accepted plan must fail validation")
    else {
        panic!("invalid accepted plan was reported as policy rejection");
    };
    assert!(diagnostic.0.contains("stack alignment"));
}

#[test]
fn register_sets_normalize_order_and_duplicates() {
    let first = RegisterSet::new([
        MachineRegister::X86R11,
        MachineRegister::X86Rax,
        MachineRegister::X86R11,
    ]);
    let second = RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R11]);
    assert_eq!(first, second);
}

#[test]
fn register_use_derives_machine_state_and_cannot_be_hidden() {
    let plan = strict_x86_entry();
    let validated = validate_boundary_entry_plan(plan, &integer_signature(1)).expect("entry plan");
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Xmm(0)]),
        MachineStateSet::empty(),
    );
    let error = validate_state_footprint(&validated, &evidence)
        .expect_err("XMM use must derive vector-state use");
    assert!(error.0.contains("ceiling"));
}

#[test]
fn call_clobbers_must_fit_the_entry_state_ceiling() {
    let mut plan = strict_x86_entry();
    plan.call.ordinary_clobbers = RegisterSet::new(
        plan.call
            .ordinary_clobbers
            .as_slice()
            .iter()
            .copied()
            .chain([MachineRegister::X86Xmm(0)]),
    );
    let error = validate_boundary_entry_plan(plan, &integer_signature(1))
        .expect_err("unsaved SIMD clobber must reject");
    assert!(error.0.contains("clobbers exceed"));
}

/// A state plan whose every field carries a distinguishable value, so a
/// reordered field shows up in the pinned bytes below.
fn pinned_state_plan(
    initial_regime: MachineRegime,
    stack: EntryStack,
    preemption: Preemption,
) -> StatePlan {
    StatePlan {
        initial_regime,
        interrupted_state: MachineStateSet::new([MachineState::GeneralRegisters]),
        saved_state: MachineStateSet::new([MachineState::VectorRegisters]),
        restored_state: MachineStateSet::new([MachineState::Flags]),
        permitted_transitive_use: MachineStateSet::new([MachineState::InstructionPointer]),
        stack,
        preemption,
    }
}

/// The four state sets, each a little-endian `u16` bitmask, in their pinned
/// order: interrupted, saved, restored, permitted transitive use.
const PINNED_STATE_SETS: [u8; 8] = [0x01, 0, 0x02, 0, 0x04, 0, 0x08, 0];

#[test]
fn state_plan_identity_bytes_are_pinned_for_every_variant() {
    // Expected bytes ported from the copies of this encoder that
    // `legalized_operations::identity::normalized_foreign` and the
    // register-home fixed-view-copy codec carried before de-duplication.
    let regimes: [(MachineRegime, &[u8]); 2] = [
        (MachineRegime::X86Long64, &[1]),
        (MachineRegime::Aarch64A64 { exception_level: 2 }, &[2, 2]),
    ];
    let stacks: [(EntryStack, &[u8]); 3] = [
        (EntryStack::Interrupted, &[1]),
        (EntryStack::Dedicated { class: 0x0403 }, &[2, 0x03, 0x04]),
        (EntryStack::ProviderSelected, &[3]),
    ];
    let preemptions: [(Preemption, &[u8]); 4] = [
        (Preemption::NotApplicable, &[1]),
        (Preemption::Masked, &[2]),
        (
            Preemption::Nestable {
                maximum_depth: 0x0605,
            },
            &[3, 0x05, 0x06],
        ),
        (Preemption::ProviderDefined, &[4]),
    ];
    for (regime, regime_bytes) in regimes {
        for (stack, stack_bytes) in stacks {
            for (preemption, preemption_bytes) in preemptions {
                let mut expected = regime_bytes.to_vec();
                expected.extend_from_slice(&PINNED_STATE_SETS);
                expected.extend_from_slice(stack_bytes);
                expected.extend_from_slice(preemption_bytes);
                let mut bytes = Vec::new();
                encode_state_plan_identity(
                    &mut bytes,
                    &pinned_state_plan(regime, stack, preemption),
                );
                assert_eq!(bytes, expected, "{regime:?} {stack:?} {preemption:?}");
            }
        }
    }
}
