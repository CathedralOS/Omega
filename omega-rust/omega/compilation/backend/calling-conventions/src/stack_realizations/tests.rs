//! Stack realization tests: arrival contexts, entry stack epochs and closures.

use super::entry_exit_stub::{X86_64ErrorCodeDisposition, derive_x86_64_entry_exit_stub};
use super::{
    ArrivalContextId, EntryStack,
    InstalledEntryFactIdentity,
    Preemption, ValidatedX86_64InstalledHardwareEntryFacts,
    X86_64ArrivalMechanism, X86_64GateKind, X86_64HardwareStackSelection,
    X86_64InstalledArrivalContext, X86_64InstalledHardwareEntryFacts, X86_64TargetProfileIdentity,
validate_x86_64_installed_hardware_entry_facts,
};

fn installed_identity() -> InstalledEntryFactIdentity {
    InstalledEntryFactIdentity {
        target_profile: X86_64TargetProfileIdentity::LONG_MODE_INTERRUPT_GATES,
        artifact: 0x20,
        installed_code: 0x30,
        entry: 0x31,
        entry_offset: 0,
        boundary_plan_report_fingerprint: 0x40,
        boundary_plan_commitment: [0x40; 32],
    }
}

fn x86_context(
    id: u64,
    interrupted_privilege: u8,
    entry_privilege: u8,
    stack_selection: X86_64HardwareStackSelection,
    mechanism: X86_64ArrivalMechanism,
) -> X86_64InstalledArrivalContext {
    X86_64InstalledArrivalContext {
        context: ArrivalContextId::new(id).expect("context identity"),
        mechanism,
        interrupted_privilege,
        entry_privilege,
        stack_selection,
        nesting: Preemption::Nestable { maximum_depth: 2 },
    }
}

fn x86_facts(
    vector: u8,
    boundary_stack: EntryStack,
    contexts: Vec<X86_64InstalledArrivalContext>,
) -> X86_64InstalledHardwareEntryFacts {
    X86_64InstalledHardwareEntryFacts {
        identity: installed_identity(),
        vector,
        gate: X86_64GateKind::Interrupt,
        boundary_stack,
        contexts,
    }
}


#[test]
fn x86_installation_validation_fails_closed_on_invalid_exact_facts() {
    let current = x86_context(
        1,
        0,
        0,
        X86_64HardwareStackSelection::Current,
        X86_64ArrivalMechanism::ExternalInterrupt,
    );
    let mut absent_identity = x86_facts(32, EntryStack::Interrupted, vec![current]);
    absent_identity.identity.artifact = 0;
    let mut absent_boundary_commitment = x86_facts(32, EntryStack::Interrupted, vec![current]);
    absent_boundary_commitment.identity.boundary_plan_commitment = [0; 32];
    let duplicate = x86_facts(32, EntryStack::Interrupted, vec![current, current]);
    let reserved_external = x86_facts(
        14,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let bad_privilege = x86_facts(
        32,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            4,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let missing_switch = x86_facts(
        32,
        EntryStack::ProviderSelected,
        vec![x86_context(
            1,
            3,
            0,
            X86_64HardwareStackSelection::Current,
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let fake_transition = x86_facts(
        32,
        EntryStack::Dedicated { class: 2 },
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::PrivilegeTransition { dedicated_class: 2 },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let bad_ist = x86_facts(
        32,
        EntryStack::Dedicated { class: 2 },
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 0,
                dedicated_class: 2,
            },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );
    let fixed_domain_drift = x86_facts(
        32,
        EntryStack::Interrupted,
        vec![x86_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 1,
                dedicated_class: 2,
            },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );

    for (facts, expected) in [
        (absent_identity, "absent exact identity"),
        (absent_boundary_commitment, "absent exact identity"),
        (duplicate, "repeats an arrival-context identity"),
        (reserved_external, "classifies reserved vector"),
        (bad_privilege, "outside 0..=3"),
        (missing_switch, "without a hardware stack switch"),
        (fake_transition, "without changing privilege"),
        (bad_ist, "invalid IST slot"),
        (fixed_domain_drift, "differs from the fixed boundary"),
    ] {
        let error = validate_x86_64_installed_hardware_entry_facts(facts)
            .expect_err("invalid x86 installation facts must reject");
        assert!(error.0.contains(expected), "{}", error.0);
    }
}
fn interrupt_boundary(class: u16) -> crate::ValidatedBoundaryEntryPlan {
    interrupt_boundary_with_parameters(class, 1)
}

fn interrupt_boundary_with_parameters(
    class: u16,
    parameter_count: usize,
) -> crate::ValidatedBoundaryEntryPlan {
    interrupt_boundary_with_shapes(
        class,
        vec![crate::ValueShape::integer(8, 8); parameter_count],
    )
}

fn interrupt_boundary_with_shapes(
    class: u16,
    parameters: Vec<crate::ValueShape>,
) -> crate::ValidatedBoundaryEntryPlan {
    let signature = crate::CallSignature {
        parameters,
        result: None,
    };
    let mut call = crate::evaluate_call_plan(crate::CallingPolicy::MicrosoftX64, &signature)
        .expect("call plan");
    call.ordinary_clobbers = crate::RegisterSet::new([
        crate::MachineRegister::X86Rax,
        crate::MachineRegister::X86Rcx,
        crate::MachineRegister::X86Rdx,
        crate::MachineRegister::X86Rsi,
        crate::MachineRegister::X86Rdi,
        crate::MachineRegister::X86R8,
        crate::MachineRegister::X86R9,
        crate::MachineRegister::X86R10,
        crate::MachineRegister::X86R11,
    ]);
    call.entry_control = crate::EntryControl::InterruptReturn;
    let interrupted = crate::MachineStateSet::new([
        crate::MachineState::GeneralRegisters,
        crate::MachineState::Flags,
        crate::MachineState::InstructionPointer,
        crate::MachineState::StackPointer,
        crate::MachineState::VectorRegisters,
    ]);
    let saved = crate::MachineStateSet::new([
        crate::MachineState::GeneralRegisters,
        crate::MachineState::Flags,
        crate::MachineState::InstructionPointer,
        crate::MachineState::StackPointer,
    ]);
    crate::validate_boundary_entry_plan(
        crate::BoundaryEntryPlan {
            call,
            state: crate::StatePlan {
                initial_regime: crate::MachineRegime::X86Long64,
                interrupted_state: interrupted,
                saved_state: saved,
                restored_state: saved,
                permitted_transitive_use: crate::MachineStateSet::new([
                    crate::MachineState::GeneralRegisters,
                    crate::MachineState::Flags,
                ]),
                stack: EntryStack::Dedicated { class },
                preemption: Preemption::Masked,
            },
        },
        &signature,
    )
    .expect("validated boundary")
}

fn stub_installed_facts(
    boundary: &crate::ValidatedBoundaryEntryPlan,
    vector: u8,
    gate: X86_64GateKind,
    contexts: Vec<X86_64InstalledArrivalContext>,
) -> ValidatedX86_64InstalledHardwareEntryFacts {
    validate_x86_64_installed_hardware_entry_facts(X86_64InstalledHardwareEntryFacts {
        identity: InstalledEntryFactIdentity {
            boundary_plan_report_fingerprint: boundary.contract_report_fingerprint(),
            boundary_plan_commitment: boundary.contract_commitment_digest(),
            ..installed_identity()
        },
        vector,
        gate,
        boundary_stack: boundary.plan().state.stack,
        contexts,
    })
    .expect("validated installed facts")
}

fn masked_context(
    id: u64,
    interrupted_privilege: u8,
    entry_privilege: u8,
    stack_selection: X86_64HardwareStackSelection,
    mechanism: X86_64ArrivalMechanism,
) -> X86_64InstalledArrivalContext {
    X86_64InstalledArrivalContext {
        context: ArrivalContextId::new(id).expect("context identity"),
        mechanism,
        interrupted_privilege,
        entry_privilege,
        stack_selection,
        nesting: Preemption::Masked,
    }
}

#[test]
fn fatal_exception_stub_derives_hardware_error_word_and_saved_state() {
    let boundary = interrupt_boundary(11);
    let installed = stub_installed_facts(
        &boundary,
        14,
        X86_64GateKind::Trap,
        vec![masked_context(
            1,
            3,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 3,
                dedicated_class: 11,
            },
            X86_64ArrivalMechanism::Exception,
        )],
    );

    let stub = derive_x86_64_entry_exit_stub(&installed, &boundary)
        .expect("the fatal-exception stub derives");

    assert_eq!(stub.stub().identity, installed.facts().identity);
    assert_eq!(stub.stub().vector, 14);
    assert_eq!(stub.stub().gate, X86_64GateKind::Trap);
    assert_eq!(
        stub.stub().exit.control,
        crate::EntryControl::InterruptReturn
    );
    assert_eq!(
        stub.stub().exit.restored_state,
        boundary.plan().state.restored_state
    );
    // The plan's saved-state law covers the general register class: the push
    // list is the fifteen non-stack GPRs, and the footprint realizes exactly
    // the saved set.
    assert_eq!(stub.stub().saved_footprint.registers().as_slice().len(), 15);
    assert_eq!(
        stub.stub().saved_footprint.machine_state(),
        boundary.plan().state.saved_state
    );
    // The member body's machine-state envelope is exactly the plan's
    // permitted transitive use — the ceiling the emitted body realizes
    // against while the stub's save area holds the interrupted state.
    assert_eq!(
        stub.stub().member_body_envelope.machine_state(),
        boundary.plan().state.permitted_transitive_use
    );
    assert_eq!(
        stub.stub()
            .member_body_envelope
            .registers()
            .as_slice()
            .len(),
        15
    );
    let [context] = stub.stub().contexts.as_slice() else {
        panic!("one derived stub context")
    };
    assert_eq!(
        context.error_code,
        X86_64ErrorCodeDisposition::HardwarePushed
    );
    assert_eq!(context.normalizing_bytes, 0);
    assert_eq!(context.hardware_frame_bytes, 48);
    assert_eq!(context.saved_area_bytes, 15 * 8);
    assert_eq!(context.mechanism, X86_64ArrivalMechanism::Exception);
    assert_eq!(context.nesting, Preemption::Masked);
}

#[test]
fn external_interrupt_stub_synthesizes_the_error_word() {
    let boundary = interrupt_boundary(7);
    let installed = stub_installed_facts(
        &boundary,
        0x20,
        X86_64GateKind::Interrupt,
        vec![masked_context(
            1,
            0,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 4,
                dedicated_class: 7,
            },
            X86_64ArrivalMechanism::ExternalInterrupt,
        )],
    );

    let stub = derive_x86_64_entry_exit_stub(&installed, &boundary)
        .expect("the timer stub derives");
    let [context] = stub.stub().contexts.as_slice() else {
        panic!("one derived stub context")
    };
    assert_eq!(
        context.error_code,
        X86_64ErrorCodeDisposition::StubSynthesized
    );
    assert_eq!(context.normalizing_bytes, 8);
    assert_eq!(context.hardware_frame_bytes, 40);
}
#[test]
fn deriver_stub_member_call_frame_reserves_indirect_copies_and_pointer_slots() {
    // A 16-byte integer rides indirectly under Microsoft x64: the pointer
    // occupies an argument register and the caller-owned copy lands in the
    // outgoing area above the register shadow space at offset 32.
    let boundary = interrupt_boundary_with_shapes(11, vec![crate::ValueShape::integer(16, 8)]);
    let installed = stub_installed_facts(
        &boundary,
        13,
        X86_64GateKind::Trap,
        vec![masked_context(
            1,
            3,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 3,
                dedicated_class: 11,
            },
            X86_64ArrivalMechanism::Exception,
        )],
    );
    let stub = derive_x86_64_entry_exit_stub(&installed, &boundary).expect("stub derives");
    assert_eq!(stub.stub().member_call_frame.outgoing_stack_bytes, 48);
    assert_eq!(stub.stub().member_call_frame.reserved_bytes, 64);

    // Five indirect parameters push the fifth pointer word onto the stack at
    // offset 32; the 16-aligned copies start at 48 and run to 128.
    let boundary = interrupt_boundary_with_shapes(11, vec![crate::ValueShape::integer(16, 8); 5]);
    let installed = stub_installed_facts(
        &boundary,
        13,
        X86_64GateKind::Trap,
        vec![masked_context(
            1,
            3,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 3,
                dedicated_class: 11,
            },
            X86_64ArrivalMechanism::Exception,
        )],
    );
    let stub = derive_x86_64_entry_exit_stub(&installed, &boundary).expect("stub derives");
    assert_eq!(stub.stub().member_call_frame.outgoing_stack_bytes, 128);
    assert_eq!(stub.stub().member_call_frame.reserved_bytes, 144);
}

#[test]
fn deriver_stub_fails_closed_on_unadmitted_boundary_or_drifted_facts() {
    let boundary = interrupt_boundary(11);
    let installed = stub_installed_facts(
        &boundary,
        13,
        X86_64GateKind::Trap,
        vec![masked_context(
            1,
            3,
            0,
            X86_64HardwareStackSelection::InterruptStackTable {
                slot: 3,
                dedicated_class: 11,
            },
            X86_64ArrivalMechanism::Exception,
        )],
    );

    // A boundary the installation did not admit: different commitment.
    let foreign = interrupt_boundary(12);
    let error = derive_x86_64_entry_exit_stub(&installed, &foreign)
        .expect_err("a lookalike policy must not attach");
    assert!(
        error.0.contains("exact admitted boundary plan"),
        "{}",
        error.0
    );

    // A boundary without interrupt-return control cannot exit a gate stub.
    let mut ordinary_call = crate::evaluate_call_plan(
        crate::CallingPolicy::MicrosoftX64,
        &crate::CallSignature {
            parameters: vec![crate::ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("call plan");
    ordinary_call.ordinary_clobbers = crate::RegisterSet::new([
        crate::MachineRegister::X86Rax,
        crate::MachineRegister::X86Rcx,
        crate::MachineRegister::X86Rdx,
    ]);
    ordinary_call.entry_control = crate::EntryControl::CallReturn;
    let saved = crate::MachineStateSet::new([
        crate::MachineState::GeneralRegisters,
        crate::MachineState::Flags,
    ]);
    let ordinary = crate::validate_boundary_entry_plan(
        crate::BoundaryEntryPlan {
            call: ordinary_call,
            state: crate::StatePlan {
                initial_regime: crate::MachineRegime::X86Long64,
                interrupted_state: saved,
                saved_state: saved,
                restored_state: saved,
                permitted_transitive_use: saved,
                stack: boundary.plan().state.stack,
                preemption: Preemption::Masked,
            },
        },
        &crate::CallSignature {
            parameters: vec![crate::ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .expect("validated ordinary boundary");
    // Give the CallReturn plan the facts' commitment to isolate the
    // entry-control rejection.
    let mismatched_control =
        validate_x86_64_installed_hardware_entry_facts(X86_64InstalledHardwareEntryFacts {
            identity: InstalledEntryFactIdentity {
                boundary_plan_report_fingerprint: ordinary.contract_report_fingerprint(),
                boundary_plan_commitment: ordinary.contract_commitment_digest(),
                ..installed_identity()
            },
            vector: 13,
            gate: X86_64GateKind::Trap,
            boundary_stack: ordinary.plan().state.stack,
            contexts: installed.facts().contexts.clone(),
        })
        .expect("validated installed facts");
    let error = derive_x86_64_entry_exit_stub(&mismatched_control, &ordinary)
        .expect_err("CallReturn cannot service an installed gate");
    assert!(error.0.contains("InterruptReturn"), "{}", error.0);

    // An arrival context carrying a different nesting than the admitted plan
    // means the facts were produced against another boundary.
    let mut drifted = installed.facts().clone();
    drifted.contexts[0].nesting = Preemption::Nestable { maximum_depth: 1 };
    let drifted = validate_x86_64_installed_hardware_entry_facts(drifted)
        .expect("drifted nesting still validates structurally");
    let error = derive_x86_64_entry_exit_stub(&drifted, &boundary)
        .expect_err("context preemption outside the plan must reject");
    assert!(error.0.contains("preemption"), "{}", error.0);
}
