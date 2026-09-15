//! Tests for calling and boundary entry plans.

use super::{
    BoundaryEntryPlan, BoundaryPlanDiagnostic, BoundaryPlanResult, CallSignature, CallingPolicy,
    CallingPolicyRejection, ConcreteVariadicCallSignature, EntryControl, EntryStack,
    IndirectPointerLocation, MachineRegime, MachineRegister, MachineState, MachineStateSet,
    Preemption, ProviderExitRealization, RegisterSet, StateFootprintEvidence, StatePlan,
    SystemVEightbyteClass, ValueLocation, ValueShape, evaluate_call_plan,
    evaluate_darwin_aapcs64_variadic_call_plan, evaluate_freestanding_program_entry_plan,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
    validate_boundary_plan_result, validate_call_plan, validate_composed_state_footprint,
    validate_provider_exit_realization, validate_runtime_value_guard_footprint,
    validate_state_footprint,
};
use target::Architecture;

fn integer_signature(parameter_count: usize) -> CallSignature {
    CallSignature {
        parameters: vec![ValueShape::integer(8, 8); parameter_count],
        result: Some(ValueShape::integer(8, 8)),
    }
}

#[test]
fn native_for_target_maps_each_declared_pair_to_its_policy() {
    for (target, policy) in [
        (
            target::NativeTarget::windows_x64(),
            CallingPolicy::MicrosoftX64,
        ),
        (
            target::NativeTarget::uefi_x64(),
            CallingPolicy::MicrosoftX64,
        ),
        (
            target::NativeTarget::linux_x64(),
            CallingPolicy::SystemVAMD64,
        ),
        (target::NativeTarget::linux_arm64(), CallingPolicy::Aapcs64),
        (target::NativeTarget::macos_arm64(), CallingPolicy::Aapcs64),
    ] {
        assert_eq!(CallingPolicy::native_for_target(target), policy);
    }
}

#[test]
fn native_for_target_resolves_a_policy_for_every_profile_target() {
    for profile in target::TargetProfile::ALL {
        let native = profile.native_target();
        assert_eq!(
            CallingPolicy::native_for_target(native).architecture(),
            native.architecture
        );
    }
}

#[test]
fn native_for_target_fails_closed_for_undeclared_pairs() {
    for (architecture, object_format) in [
        (Architecture::X86_64, target::ObjectFormat::MachO),
        (Architecture::Aarch64, target::ObjectFormat::Coff),
    ] {
        let undeclared = target::NativeTarget {
            architecture,
            object_format,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        assert!(
            std::panic::catch_unwind(|| CallingPolicy::native_for_target(undeclared)).is_err(),
            "undeclared pair must fail closed"
        );
    }
}

#[test]
fn native_syscall_for_target_maps_each_supported_target_to_its_row() {
    for (target, policy) in [
        (
            target::NativeTarget::linux_x64(),
            Some(CallingPolicy::LinuxSyscallX86_64),
        ),
        (
            target::NativeTarget::linux_arm64(),
            Some(CallingPolicy::LinuxSyscallAarch64),
        ),
        (target::NativeTarget::windows_x64(), None),
        (target::NativeTarget::uefi_x64(), None),
        (target::NativeTarget::macos_arm64(), None),
    ] {
        assert_eq!(CallingPolicy::native_syscall_for_target(target), policy);
    }
}

#[test]
fn native_syscall_for_target_resolves_for_every_profile_target() {
    for profile in target::TargetProfile::ALL {
        let native = profile.native_target();
        if let Some(policy) = CallingPolicy::native_syscall_for_target(native) {
            assert_eq!(policy.architecture(), native.architecture);
        }
    }
}

#[test]
fn native_syscall_for_target_fails_closed_for_undeclared_pairs() {
    for (architecture, object_format) in [
        (Architecture::X86_64, target::ObjectFormat::MachO),
        (Architecture::Aarch64, target::ObjectFormat::Coff),
    ] {
        let undeclared = target::NativeTarget {
            architecture,
            object_format,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        assert!(
            std::panic::catch_unwind(|| { CallingPolicy::native_syscall_for_target(undeclared) })
                .is_err(),
            "undeclared pair must fail closed"
        );
    }
}

#[test]
fn syscall_matrix_rows_evaluate_and_reject_architecture_drift() {
    let signature = integer_signature(4);
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let policy = CallingPolicy::native_syscall_for_target(target)
            .expect("linux targets declare a syscall row");
        let plan = evaluate_call_plan(policy, &signature).expect("syscall plan");
        validate_call_plan(&plan, &signature).expect("declared row validates");

        let foreign = match target.architecture {
            Architecture::X86_64 => MachineRegister::Aarch64X(0),
            Architecture::Aarch64 => MachineRegister::X86Rax,
        };
        let mut drifted = plan.clone();
        drifted.ordinary_clobbers = RegisterSet::new([foreign]);
        assert!(
            validate_call_plan(&drifted, &signature).is_err(),
            "a clobber from the wrong architecture must reject"
        );

        let mut drifted = plan;
        drifted.entry_control = EntryControl::SupervisorCall {
            number_register: foreign,
            immediate: 0,
        };
        assert!(
            validate_call_plan(&drifted, &signature).is_err(),
            "an entry-control register from the wrong architecture must reject"
        );
    }
}

#[test]
fn microsoft_x64_places_four_registers_then_shadow_relative_stack() {
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &integer_signature(6))
        .expect("MS x64 plan");
    assert!(matches!(
        plan.parameters[0].locations[0],
        ValueLocation::Register {
            register: MachineRegister::X86Rcx,
            ..
        }
    ));
    assert!(matches!(
        plan.parameters[4].locations[0],
        ValueLocation::Stack {
            stack_byte_offset: 32,
            ..
        }
    ));
    assert_eq!(plan.shadow_bytes, 32);
}

#[test]
fn zero_sized_parameters_retain_identity_without_consuming_abi_locations() {
    let empty = ValueShape::integer(0, 1);
    let scalar = ValueShape::integer(8, 8);
    for policy in [
        CallingPolicy::MicrosoftX64,
        CallingPolicy::SystemVAMD64,
        CallingPolicy::Aapcs64,
    ] {
        let plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![empty, scalar, empty],
                result: None,
            },
        )
        .expect("canonical empty values are erased from ABI placement");
        assert_eq!(plan.parameters.len(), 3);
        assert_eq!(plan.parameters[0].shape, empty);
        assert!(plan.parameters[0].locations.is_empty());
        assert_eq!(plan.parameters[1].shape, scalar);
        assert!(!plan.parameters[1].locations.is_empty());
        assert_eq!(plan.parameters[2].shape, empty);
        assert!(plan.parameters[2].locations.is_empty());
        validate_call_plan(
            &plan,
            &CallSignature {
                parameters: vec![empty, scalar, empty],
                result: None,
            },
        )
        .unwrap();
    }
}

#[test]
fn microsoft_x64_indirect_result_uses_rcx_and_shifts_parameters() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 4],
        result: Some(ValueShape::integer(16, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("MS x64 indirect-result plan");

    assert!(matches!(
        plan.result.expect("indirect result").locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }]
    ));
    assert!(matches!(
        plan.parameters[0].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Rdx,
            ..
        }]
    ));
    assert!(matches!(
        plan.parameters[3].locations.as_slice(),
        [ValueLocation::Stack {
            stack_byte_offset: 32,
            ..
        }]
    ));
}

#[test]
fn microsoft_x64_indirect_parameters_use_positional_pointer_slots() {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(16, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(16, 8),
        ],
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
        .expect("MS x64 indirect-parameter plan");

    assert!(matches!(
        plan.parameters[0].locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: Some(48),
            ..
        }]
    ));
    assert!(matches!(
        plan.parameters[4].locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 32,
                alignment: 8,
            },
            copy_stack_byte_offset: Some(64),
            ..
        }]
    ));
}

#[test]
fn system_v_and_aapcs_use_independent_float_register_banks() {
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(8, 8),
            ValueShape::float(8),
            ValueShape::integer(8, 8),
            ValueShape::float(8),
        ],
        result: None,
    };
    let sysv = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).expect("SysV");
    let aapcs = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS");
    assert!(matches!(
        sysv.parameters[1].locations[0],
        ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            ..
        }
    ));
    assert!(matches!(
        aapcs.parameters[3].locations[0],
        ValueLocation::Register {
            register: MachineRegister::Aarch64V(1),
            ..
        }
    ));
}

#[test]
fn darwin_aapcs64_variadic_scalars_start_on_the_stack() {
    let word = ValueShape::integer(8, 8);
    let mode = ValueShape::integer(4, 4);
    let signature = ConcreteVariadicCallSignature {
        fixed_parameters: vec![word, ValueShape::integer(4, 4)],
        variadic_parameters: vec![mode],
        result: Some(ValueShape::integer(4, 4)),
    };
    let plan =
        evaluate_darwin_aapcs64_variadic_call_plan(&signature).expect("Darwin arm64 variadic plan");

    assert!(matches!(
        plan.parameters[0].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            ..
        }]
    ));
    assert!(matches!(
        plan.parameters[1].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(1),
            ..
        }]
    ));
    assert_eq!(
        plan.parameters[2].locations,
        vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 4,
            alignment: 8,
        }]
    );
    assert!(matches!(
        plan.result.expect("result placement").locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(0),
            ..
        }]
    ));
}

#[test]
fn system_v_small_integer_aggregates_use_consecutive_registers_and_results() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 8)],
        result: Some(ValueShape::integer(12, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV small aggregate plan");

    assert_eq!(
        plan.parameters[1].locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Rsi,
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    assert_eq!(
        plan.result.expect("aggregate result").locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                value_byte_offset: 8,
                byte_size: 4,
            },
        ]
    );
}

#[test]
fn system_v_register_exhausted_aggregate_moves_wholly_to_stack_and_rolls_back() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 5]
            .into_iter()
            .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
            .collect(),
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV exhausted aggregate plan");

    assert_eq!(
        plan.parameters[5].locations,
        vec![
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    assert_eq!(
        plan.parameters[6].locations,
        vec![ValueLocation::Register {
            register: MachineRegister::X86R9,
            value_byte_offset: 0,
            byte_size: 8,
        }]
    );
}

#[test]
fn aapcs_hfa_is_one_value_split_across_vector_registers() {
    let signature = CallSignature {
        parameters: vec![ValueShape::homogeneous_float_aggregate(8, 4)],
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS HFA");
    assert_eq!(plan.parameters[0].locations.len(), 4);
    assert!(matches!(
        plan.parameters[0].locations[3],
        ValueLocation::Register {
            register: MachineRegister::Aarch64V(3),
            value_byte_offset: 24,
            ..
        }
    ));
}

#[test]
fn aapcs_small_integer_aggregates_use_consecutive_x_registers() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(16, 16)],
        result: Some(ValueShape::integer(12, 8)),
    };
    let plan =
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS small aggregate plan");

    assert_eq!(
        plan.parameters[1].locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(2),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(3),
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    assert_eq!(
        plan.result.expect("aggregate result").locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(0),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::Aarch64X(1),
                value_byte_offset: 8,
                byte_size: 4,
            },
        ]
    );
}

#[test]
fn aapcs_small_aggregate_moves_wholly_to_stack_when_x_registers_run_out() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 7]
            .into_iter()
            .chain([ValueShape::integer(16, 8), ValueShape::integer(8, 8)])
            .collect(),
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
        .expect("AAPCS exhausted aggregate plan");

    assert_eq!(
        plan.parameters[7].locations,
        vec![
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    assert!(matches!(
        plan.parameters[8].locations.as_slice(),
        [ValueLocation::Stack {
            stack_byte_offset: 16,
            ..
        }]
    ));
}

#[test]
fn aapcs_large_aggregates_use_caller_copies_and_indirect_results() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(32, 16)),
    };
    let plan =
        evaluate_call_plan(CallingPolicy::Aapcs64, &signature).expect("AAPCS large aggregate plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(0)),
            copy_stack_byte_offset: Some(0),
            byte_size: 24,
            alignment: 8,
        }]
    );
    assert!(matches!(
        plan.parameters[1].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::Aarch64X(1),
            ..
        }]
    ));
    assert_eq!(
        plan.result.expect("indirect aggregate result").locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::Aarch64X(8)),
            copy_stack_byte_offset: None,
            byte_size: 32,
            alignment: 16,
        }]
    );
}

#[test]
fn aapcs_large_aggregate_pointer_uses_stack_before_its_copy() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); 8]
            .into_iter()
            .chain([ValueShape::integer(24, 16)])
            .collect(),
        result: None,
    };
    let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
        .expect("AAPCS stack-indirect aggregate plan");

    assert_eq!(
        plan.parameters[8].locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            copy_stack_byte_offset: Some(16),
            byte_size: 24,
            alignment: 16,
        }]
    );
}

#[test]
fn linux_syscall_pins_non_c_call_registers() {
    let plan = evaluate_call_plan(CallingPolicy::LinuxSyscallX86_64, &integer_signature(6))
        .expect("Linux syscall");
    assert!(matches!(
        plan.parameters[3].locations[0],
        ValueLocation::Register {
            register: MachineRegister::X86R10,
            ..
        }
    ));
    assert!(matches!(
        plan.entry_control,
        EntryControl::SupervisorCall {
            number_register: MachineRegister::X86Rax,
            immediate: 0,
        }
    ));

    let aarch64 = evaluate_call_plan(CallingPolicy::LinuxSyscallAarch64, &integer_signature(6))
        .expect("AArch64 Linux syscall");
    assert!(
        aarch64
            .ordinary_clobbers
            .contains(MachineRegister::Aarch64X(8)),
        "the number register is part of the realized syscall clobber set"
    );
}

fn strict_x86_entry() -> BoundaryEntryPlan {
    let mut call =
        evaluate_call_plan(CallingPolicy::MicrosoftX64, &integer_signature(1)).expect("call plan");
    call.ordinary_clobbers = RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ]);
    call.entry_control = EntryControl::InterruptReturn;
    let interrupted = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::VectorRegisters,
    ]);
    let saved = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    BoundaryEntryPlan {
        call,
        state: StatePlan {
            initial_regime: MachineRegime::X86Long64,
            interrupted_state: interrupted,
            saved_state: saved,
            restored_state: saved,
            permitted_transitive_use: MachineStateSet::new([
                MachineState::GeneralRegisters,
                MachineState::Flags,
            ]),
            stack: EntryStack::Dedicated { class: 1 },
            preemption: Preemption::Masked,
        },
    }
}

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

#[test]
fn system_v_memory_class_uses_stack_values_and_a_hidden_result_pointer() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(24, 8), ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(24, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV MEMORY-class plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 16,
                value_byte_offset: 16,
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    assert!(matches!(
        plan.parameters[1].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Rsi,
            ..
        }]
    ));
    assert!(matches!(
        plan.result.expect("indirect result").locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdi),
            copy_stack_byte_offset: None,
            byte_size: 24,
            alignment: 8,
        }]
    ));
}

#[test]
fn system_v_two_f64_record_uses_sse_fragments_or_whole_stack_rollback() {
    let pair = ValueShape::homogeneous_float_aggregate(8, 2);
    let signature = CallSignature {
        parameters: vec![ValueShape::float(8); 8]
            .into_iter()
            .chain([pair])
            .collect(),
        result: Some(pair),
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
        .expect("SysV two-f64 record plan");

    assert_eq!(
        plan.parameters[8].locations,
        vec![
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    assert!(matches!(
        plan.result.expect("SSE result").locations.as_slice(),
        [
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                ..
            },
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(1),
                ..
            }
        ]
    ));
}

#[test]
fn system_v_three_f32_record_packs_into_two_sse_eightbytes() {
    let triple = ValueShape::homogeneous_float_aggregate(4, 3);
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![triple],
            result: Some(triple),
        },
    )
    .expect("SysV three-f32 record plan");

    let expected = vec![
        ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            value_byte_offset: 0,
            byte_size: 8,
        },
        ValueLocation::Register {
            register: MachineRegister::X86Xmm(1),
            value_byte_offset: 8,
            byte_size: 4,
        },
    ];
    assert_eq!(plan.parameters[0].locations, expected);
    assert_eq!(plan.result.expect("packed SSE result").locations, expected);
}

#[test]
fn system_v_mixed_record_uses_independent_register_banks() {
    let integer_sse = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse,
    );
    let sse_integer = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Sse,
        SystemVEightbyteClass::Integer,
    );
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![integer_sse],
            result: Some(sse_integer),
        },
    )
    .expect("mixed SysV aggregate plan");

    assert_eq!(
        plan.parameters[0].locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Rdi,
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
    assert_eq!(
        plan.result.expect("mixed result").locations,
        vec![
            ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                value_byte_offset: 0,
                byte_size: 8,
            },
            ValueLocation::Register {
                register: MachineRegister::X86Rax,
                value_byte_offset: 8,
                byte_size: 8,
            },
        ]
    );
}

#[test]
fn system_v_mixed_record_rolls_back_both_banks() {
    let mixed = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Sse,
    );
    let mut parameters = vec![ValueShape::float(8); 8];
    parameters.extend([mixed, ValueShape::integer(8, 8)]);
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .expect("mixed SysV rollback plan");

    assert!(matches!(
        plan.parameters[8].locations.as_slice(),
        [
            ValueLocation::Stack {
                stack_byte_offset: 0,
                value_byte_offset: 0,
                byte_size: 8,
                ..
            },
            ValueLocation::Stack {
                stack_byte_offset: 8,
                value_byte_offset: 8,
                byte_size: 8,
                ..
            }
        ]
    ));
    assert!(matches!(
        plan.parameters[9].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Rdi,
            ..
        }]
    ));

    let mut parameters = vec![ValueShape::integer(8, 8); 6];
    parameters.extend([mixed, ValueShape::float(8)]);
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .expect("inverse mixed SysV rollback plan");
    assert!(
        plan.parameters[6]
            .locations
            .iter()
            .all(|location| matches!(location, ValueLocation::Stack { .. }))
    );
    assert!(matches!(
        plan.parameters[7].locations.as_slice(),
        [ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            ..
        }]
    ));
}

#[test]
fn system_v_classified_record_rejects_all_integer_eightbytes() {
    let malformed = ValueShape::system_v_aggregate(
        16,
        8,
        SystemVEightbyteClass::Integer,
        SystemVEightbyteClass::Integer,
    );
    let error = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![malformed],
            result: None,
        },
    )
    .expect_err("equal classes must use an existing normalized aggregate class");
    assert!(error.0.contains("at least one SSE eightbyte"));
}

#[test]
fn borrowed_references_retain_original_storage_pointers_without_copies() {
    let borrowed = ValueShape::borrowed_reference(16, 8);
    for (policy, register) in [
        (CallingPolicy::MicrosoftX64, MachineRegister::X86Rcx),
        (CallingPolicy::SystemVAMD64, MachineRegister::X86Rdi),
        (CallingPolicy::Aapcs64, MachineRegister::Aarch64X(0)),
    ] {
        let plan = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![borrowed],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("borrowed-reference plan");
        assert_eq!(
            plan.parameters[0].locations,
            vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                byte_size: 16,
                alignment: 8,
            }]
        );
        validate_call_plan(
            &plan,
            &CallSignature {
                parameters: vec![borrowed],
                result: Some(ValueShape::integer(4, 4)),
            },
        )
        .expect("borrowed-reference plan validates");
    }
}

#[test]
fn zero_byte_referents_retain_pointer_abi_and_reject_erased_pointer_evidence() {
    for policy in [
        CallingPolicy::MicrosoftX64,
        CallingPolicy::SystemVAMD64,
        CallingPolicy::Aapcs64,
    ] {
        let signature = CallSignature {
            parameters: vec![
                ValueShape::integer(0, 1),
                ValueShape::borrowed_reference(0, 1),
                ValueShape::integer(8, 8),
            ],
            result: None,
        };
        let plan =
            evaluate_call_plan(policy, &signature).expect("empty referent still needs pointer");
        assert!(plan.parameters[0].locations.is_empty());
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Indirect {
                copy_stack_byte_offset: None,
                byte_size: 0,
                alignment: 1,
                ..
            }]
        ));
        assert!(!plan.parameters[2].locations.is_empty());
        let nonempty = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: vec![
                    ValueShape::borrowed_reference(4, 4),
                    ValueShape::integer(8, 8),
                ],
                result: None,
            },
        )
        .unwrap();
        assert_eq!(
            plan.parameters[2], nonempty.parameters[1],
            "empty referent consumes the same pointer slot"
        );
        let mut missing_pointer = plan.clone();
        missing_pointer.parameters[1].locations.clear();
        assert!(validate_call_plan(&missing_pointer, &signature).is_err());
        let mut copied = plan.clone();
        let ValueLocation::Indirect {
            copy_stack_byte_offset,
            ..
        } = &mut copied.parameters[1].locations[0]
        else {
            panic!("pointer placement")
        };
        *copy_stack_byte_offset = Some(0);
        assert!(validate_call_plan(&copied, &signature).is_err());
        for invalid in [ValueShape::integer(0, 8), ValueShape::float(0)] {
            assert!(
                evaluate_call_plan(
                    policy,
                    &CallSignature {
                        parameters: vec![invalid],
                        result: None
                    }
                )
                .is_err()
            );
        }
    }
}

#[test]
fn borrowed_reference_stack_pointer_is_not_a_referent_copy() {
    let mut parameters = vec![ValueShape::integer(8, 8); 6];
    parameters.push(ValueShape::borrowed_reference(16, 8));
    let plan = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters,
            result: None,
        },
    )
    .expect("stacked borrowed-reference plan");
    assert_eq!(
        plan.parameters[6].locations,
        vec![ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            copy_stack_byte_offset: None,
            byte_size: 16,
            alignment: 8,
        }]
    );
}

#[test]
fn borrowed_references_are_not_results() {
    let error = evaluate_call_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::borrowed_reference(8, 8)),
        },
    )
    .expect_err("borrowed-reference result must be rejected");
    assert!(error.0.contains("parameter-only"));
}
