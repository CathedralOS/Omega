use super::*;
use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegime, MachineState, MachineStateSet, StatePlan,
    evaluate_call_plan, validate_boundary_entry_plan,
};
use iced_x86::{Decoder, DecoderOptions, Mnemonic};

fn permitted() -> MachineStateSet {
    MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ])
}

fn request(stack: EntryStack) -> NativeEntryBridgeRequest {
    let target = NativeTarget::linux_x64();
    let policy = CallingPolicy::native_for_target(target);
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let mut call = evaluate_call_plan(policy, &signature).unwrap();
    call.entry_control = EntryControl::InterruptReturn;
    call.ordinary_clobbers = RegisterSet::default();
    let inbound = validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime: MachineRegime::X86Long64,
                interrupted_state: MachineStateSet::empty(),
                saved_state: MachineStateSet::empty(),
                restored_state: MachineStateSet::empty(),
                permitted_transitive_use: permitted(),
                stack,
                preemption: Preemption::Masked,
            },
        },
        &signature,
    )
    .unwrap();
    let internal = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(16, 8)],
            result: None,
        },
    )
    .unwrap();
    let mut parameters = Arena::with_capacity(1);
    parameters.insert(NativeEntryParameter {
        inbound_ordinal: 0,
        scalar_codec: 0,
    });
    NativeEntryBridgeRequest {
        target,
        internal,
        inbound: inbound.plan().clone(),
        body_footprint: Some(StateFootprintEvidence::new(
            RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rsp]),
            permitted(),
        )),
        parameters,
    }
}

fn plan() -> InstalledInterruptEntryPlan {
    InstalledInterruptEntryPlan::fatal_context(512, 0, 4096).unwrap()
}

#[test]
fn fatal_arrival_emits_and_replays_without_a_returning_continuation() {
    for stack in [EntryStack::Interrupted, EntryStack::Dedicated { class: 2 }] {
        let bridge = emit(request(stack), plan()).unwrap();
        let instructions = Decoder::new(64, bridge.bytes(), DecoderOptions::NONE)
            .into_iter()
            .collect::<Vec<_>>();
        assert!(
            instructions
                .iter()
                .all(|instruction| !instruction.is_invalid())
        );
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.mnemonic() == Mnemonic::Cld)
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| instruction.mnemonic() == Mnemonic::Call)
                .count(),
            1
        );
        assert!(
            instructions.iter().all(|instruction| !matches!(
                instruction.mnemonic(),
                Mnemonic::Iretq | Mnemonic::Ret
            ))
        );
        assert!(bridge.bytes().ends_with(&isa_x86_64::PRIVILEGED_STOP_BYTES));
        assert_eq!(bridge.installed_interrupt(), Some(plan()));
        assert!(
            bridge.frame_bytes() >= plan().hardware_frame_bytes() + plan().saved_register_bytes
        );
        validate_native_entry_bridge(&bridge).unwrap();
        let mut altered = bridge;
        altered.bytes[0] ^= 1;
        assert_eq!(
            validate_native_entry_bridge(&altered),
            Err(Error::ReplayMismatch)
        );
    }
}

#[test]
fn fatal_arrival_refuses_unrealized_preservation_promises() {
    let mut interrupted = request(EntryStack::Interrupted);
    interrupted.inbound.state.interrupted_state = permitted();
    let mut saved = request(EntryStack::Interrupted);
    saved.inbound.state.saved_state = permitted();
    let mut restored = request(EntryStack::Interrupted);
    restored.inbound.state.restored_state = permitted();
    for request in [interrupted, saved, restored] {
        assert_eq!(
            emit(request, plan()),
            Err(Error::UnsupportedFatalInterruptPreservation)
        );
    }
}

#[test]
fn fatal_arrival_keeps_complete_body_state_evidence() {
    let mut missing = request(EntryStack::Interrupted);
    missing.body_footprint = None;
    assert_eq!(
        emit(missing, plan()),
        Err(Error::MissingInterruptBodyFootprint)
    );
    let mut outside_ceiling = request(EntryStack::Interrupted);
    outside_ceiling.inbound.state.permitted_transitive_use =
        MachineStateSet::new([MachineState::GeneralRegisters]);
    let mut vector = request(EntryStack::Interrupted);
    vector.body_footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Xmm(0)]),
        permitted(),
    ));
    for request in [outside_ceiling, vector] {
        assert_eq!(
            emit(request, plan()),
            Err(Error::InterruptBodyStateExceedsPlan)
        );
    }
}

#[test]
fn fatal_arrival_does_not_relax_mask_stack_or_returning_timer_protocols() {
    let mut unmasked = request(EntryStack::Interrupted);
    unmasked.inbound.state.preemption = Preemption::NotApplicable;
    let unselected = request(EntryStack::ProviderSelected);
    let mut ordinary = request(EntryStack::Interrupted);
    ordinary.inbound.call.entry_control = EntryControl::CallReturn;
    for request in [unmasked, unselected, ordinary] {
        assert_eq!(emit(request, plan()), Err(Error::UnsupportedInterruptState));
    }
    assert_eq!(
        emit_installed_interrupt_entry_bridge(
            request(EntryStack::Dedicated { class: 1 }),
            InstalledInterruptEntryPlan::new(InstalledInterruptFrame::NoErrorCode),
        ),
        Err(Error::UnsupportedInterruptState),
    );
}
