use omega_calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use omega_target::{NativeTarget, TargetProfile};

use super::*;

fn context() -> NativeFuelContextLayout {
    NativeFuelContextLayout {
        byte_size: 112,
        alignment: 16,
        remaining_units_offset: 0,
        unpaid_site_kind_offset: 8,
        unpaid_site_identity_offset: 16,
        required_units_offset: 24,
        transfer_entry_offset: 32,
        retry_code_offset_offset: 40,
        sponsor_stack_top_offset: 48,
        activation_state_offset: 64,
        activation_state_byte_count: 40,
    }
}

fn slots(architecture: omega_target::Architecture) -> Vec<NativeFuelActivationStateSlot> {
    let (general, vector) = match architecture {
        omega_target::Architecture::X86_64 => (MachineRegister::X86Rax, MachineRegister::X86Xmm(0)),
        omega_target::Architecture::Aarch64 => {
            (MachineRegister::Aarch64X(0), MachineRegister::Aarch64V(0))
        }
    };
    vec![
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(general),
            context_offset: 64,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Flags,
            context_offset: 72,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(vector),
            context_offset: 80,
            byte_count: 16,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::StackPointer,
            context_offset: 96,
            byte_count: 8,
        },
    ]
}

fn states() -> MachineStateSet {
    MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ])
}

fn transfer_entry() -> NativeFuelRuntimeEntryIdentity {
    NativeFuelRuntimeEntryIdentity {
        section_identity: 1,
        symbol_identity: 2,
    }
}

fn resume_entry() -> NativeFuelRuntimeEntryIdentity {
    NativeFuelRuntimeEntryIdentity {
        section_identity: 1,
        symbol_identity: 3,
    }
}

fn stack() -> NativeFuelSponsorStackPlan {
    NativeFuelSponsorStackPlan {
        alignment: 16,
        byte_ceiling: 256,
    }
}

fn plan_for(profile: TargetProfile) -> NativeFuelTransferRuntimePlanProjection {
    let target = profile.native_target();
    let transport = match target.architecture {
        omega_target::Architecture::X86_64 => {
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            }
        }
        omega_target::Architecture::Aarch64 => {
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
            }
        }
    };
    NativeFuelTransferRuntimePlanProjection::new(
        profile,
        target,
        transport,
        context(),
        slots(target.architecture),
        stack(),
        states(),
        states(),
        states(),
        transfer_entry(),
        resume_entry(),
    )
    .expect("canonical transfer-runtime plan")
}

fn x86_plan() -> NativeFuelTransferRuntimePlanProjection {
    plan_for(TargetProfile::LinuxX64)
}

#[test]
fn x86_and_aarch64_plans_are_canonical_and_nonzero() {
    for profile in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let plan = plan_for(profile);
        assert_eq!(plan.profile(), profile);
        assert_eq!(plan.target(), profile.native_target());
        assert_eq!(
            plan.activation_state_slots(),
            slots(plan.target().architecture)
        );
        assert_eq!(plan.interrupted_state(), states());
        assert_eq!(plan.saved_state(), states());
        assert_eq!(plan.restored_state(), states());
        assert_ne!(plan.normalized_identity(), 0);

        let policy = NativeFuelTargetPlanProjection {
            profile,
            target: plan.target(),
            transport: plan.transport(),
            context: plan.context(),
            transfer_plan_identity: plan.normalized_identity(),
        };
        assert_eq!(plan.validate_target_policy(policy), Ok(()));
        assert_eq!(
            NativeFuelTransferRuntimePlanProjection::from_target_policy(
                policy,
                slots(plan.target().architecture),
                stack(),
                states(),
                states(),
                states(),
                transfer_entry(),
                resume_entry(),
            ),
            Ok(plan)
        );
    }
}

#[test]
fn every_valid_structural_mutation_changes_plan_identity() {
    let baseline = plan_for(TargetProfile::WindowsX64);
    let baseline_identity = baseline.normalized_identity();
    let mut variants = Vec::new();

    variants.push(plan_for(TargetProfile::UefiX64));

    let mut changed_slots = slots(baseline.target().architecture);
    changed_slots[0].value = NativeFuelSavedValue::Register(MachineRegister::X86Rcx);
    variants.push(
        NativeFuelTransferRuntimePlanProjection::new(
            baseline.profile(),
            baseline.target(),
            baseline.transport(),
            baseline.context(),
            changed_slots,
            stack(),
            states(),
            states(),
            states(),
            transfer_entry(),
            resume_entry(),
        )
        .unwrap(),
    );

    variants.push(
        NativeFuelTransferRuntimePlanProjection::new(
            baseline.profile(),
            baseline.target(),
            baseline.transport(),
            baseline.context(),
            slots(baseline.target().architecture),
            NativeFuelSponsorStackPlan {
                alignment: 16,
                byte_ceiling: 512,
            },
            states(),
            states(),
            states(),
            transfer_entry(),
            resume_entry(),
        )
        .unwrap(),
    );
    variants.push(
        NativeFuelTransferRuntimePlanProjection::new(
            baseline.profile(),
            baseline.target(),
            baseline.transport(),
            baseline.context(),
            slots(baseline.target().architecture),
            stack(),
            states(),
            states(),
            states(),
            NativeFuelRuntimeEntryIdentity {
                symbol_identity: 4,
                ..transfer_entry()
            },
            resume_entry(),
        )
        .unwrap(),
    );
    variants.push(
        NativeFuelTransferRuntimePlanProjection::new(
            baseline.profile(),
            baseline.target(),
            baseline.transport(),
            baseline.context(),
            slots(baseline.target().architecture),
            stack(),
            states(),
            states(),
            states(),
            transfer_entry(),
            NativeFuelRuntimeEntryIdentity {
                section_identity: 2,
                ..resume_entry()
            },
        )
        .unwrap(),
    );

    assert!(
        variants
            .iter()
            .all(|variant| variant.normalized_identity() != baseline_identity)
    );
}

#[test]
fn malformed_target_transport_and_context_recipes_reject() {
    let build = |profile, target, transport, context| {
        NativeFuelTransferRuntimePlanProjection::new(
            profile,
            target,
            transport,
            context,
            slots(target.architecture),
            stack(),
            states(),
            states(),
            states(),
            transfer_entry(),
            resume_entry(),
        )
    };
    let invalid_width = NativeTarget {
        pointer_size: 4,
        pointer_alignment: 4,
        ..NativeTarget::linux_x64()
    };
    assert_eq!(
        build(
            TargetProfile::LinuxX64,
            invalid_width,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx
            },
            context(),
        ),
        Err(NativeFuelTransferPlanError::InvalidTargetRecipe)
    );
    assert_eq!(
        build(
            TargetProfile::LinuxX64,
            NativeTarget::linux_x64(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86R12
            },
            context(),
        ),
        Err(NativeFuelTransferPlanError::InvalidTargetRecipe)
    );

    for mutate in [
        |context: &mut NativeFuelContextLayout| context.alignment = 3,
        |context: &mut NativeFuelContextLayout| context.remaining_units_offset = 1,
        |context: &mut NativeFuelContextLayout| context.activation_state_offset = 48,
        |context: &mut NativeFuelContextLayout| context.activation_state_byte_count = 0,
    ] {
        let mut changed = context();
        mutate(&mut changed);
        assert_eq!(
            build(
                TargetProfile::LinuxX64,
                NativeTarget::linux_x64(),
                SponsorContextTransport::ReservedNonvolatileRegister {
                    register: MachineRegister::X86Rbx
                },
                changed,
            ),
            Err(NativeFuelTransferPlanError::InvalidTargetRecipe)
        );
    }
}

#[test]
fn activation_slot_mutations_reject() {
    let build = |slots| {
        NativeFuelTransferRuntimePlanProjection::new(
            TargetProfile::LinuxX64,
            NativeTarget::linux_x64(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
            context(),
            slots,
            stack(),
            states(),
            states(),
            states(),
            transfer_entry(),
            resume_entry(),
        )
    };

    assert_eq!(
        build(Vec::new()),
        Err(NativeFuelTransferPlanError::EmptyActivationState)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[0].byte_count = 4;
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::InvalidActivationStateSlot)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[0].value = NativeFuelSavedValue::Register(MachineRegister::Aarch64X(0));
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::InvalidActivationStateSlot)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[2].context_offset = 88;
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::InvalidActivationStateSlot)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[1].value = changed[0].value;
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::DuplicateSavedValue)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[1].context_offset = 64;
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::NonCanonicalActivationStateSlots)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[1].context_offset = 80;
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::IncompleteActivationStateCoverage)
    );
    let mut changed = slots(omega_target::Architecture::X86_64);
    changed[3].value = NativeFuelSavedValue::Register(MachineRegister::X86Rdx);
    assert_eq!(
        build(changed),
        Err(NativeFuelTransferPlanError::StateSetMismatch)
    );
}

#[test]
fn state_stack_entry_and_policy_mutations_reject() {
    let baseline = x86_plan();
    let incomplete = MachineStateSet::new([
        MachineState::GeneralRegisters,
        MachineState::Flags,
        MachineState::InstructionPointer,
        MachineState::StackPointer,
    ]);
    let rebuild = |stack, interrupted, saved, restored, transfer, resume| {
        NativeFuelTransferRuntimePlanProjection::new(
            baseline.profile(),
            baseline.target(),
            baseline.transport(),
            baseline.context(),
            slots(baseline.target().architecture),
            stack,
            interrupted,
            saved,
            restored,
            transfer,
            resume,
        )
    };
    for state_sets in [
        (incomplete, states(), states()),
        (states(), incomplete, states()),
        (states(), states(), incomplete),
    ] {
        assert_eq!(
            rebuild(
                stack(),
                state_sets.0,
                state_sets.1,
                state_sets.2,
                transfer_entry(),
                resume_entry()
            ),
            Err(NativeFuelTransferPlanError::StateSetMismatch)
        );
    }
    for invalid_stack in [
        NativeFuelSponsorStackPlan {
            alignment: 3,
            byte_ceiling: 256,
        },
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 0,
        },
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 255,
        },
    ] {
        assert_eq!(
            rebuild(
                invalid_stack,
                states(),
                states(),
                states(),
                transfer_entry(),
                resume_entry()
            ),
            Err(NativeFuelTransferPlanError::InvalidSponsorStack)
        );
    }
    assert_eq!(
        rebuild(
            stack(),
            states(),
            states(),
            states(),
            NativeFuelRuntimeEntryIdentity {
                section_identity: 0,
                symbol_identity: 2
            },
            resume_entry(),
        ),
        Err(NativeFuelTransferPlanError::InvalidEntryIdentity)
    );
    assert_eq!(
        rebuild(
            stack(),
            states(),
            states(),
            states(),
            transfer_entry(),
            transfer_entry()
        ),
        Err(NativeFuelTransferPlanError::DuplicateEntryIdentity)
    );

    let wrong_identity = NativeFuelTargetPlanProjection {
        profile: baseline.profile(),
        target: baseline.target(),
        transport: baseline.transport(),
        context: baseline.context(),
        transfer_plan_identity: baseline.normalized_identity().wrapping_add(1),
    };
    assert!(matches!(
        baseline.validate_target_policy(wrong_identity),
        Err(NativeFuelTransferPlanError::TransferPlanIdentityMismatch { .. })
    ));
    assert_eq!(
        baseline.validate_target_policy(NativeFuelTargetPlanProjection {
            context: NativeFuelContextLayout {
                byte_size: 128,
                ..baseline.context()
            },
            transfer_plan_identity: baseline.normalized_identity(),
            ..wrong_identity
        }),
        Err(NativeFuelTransferPlanError::TargetPolicyMismatch)
    );
}

fn text(
    entry: NativeFuelRuntimeEntryIdentity,
    offset: usize,
    bytes: Vec<u8>,
) -> NativeFuelRuntimeTextEvidence {
    NativeFuelRuntimeTextEvidence::new(
        entry,
        NativeFuelRuntimeTextSpan {
            text_offset: offset,
            byte_count: bytes.len(),
        },
        bytes.clone(),
        bytes,
    )
    .expect("well-formed runtime text evidence")
}

fn footprint(register: MachineRegister) -> StateFootprintEvidence {
    StateFootprintEvidence::new(
        RegisterSet::new([
            register,
            MachineRegister::X86Rsp,
            MachineRegister::X86Xmm(0),
        ]),
        MachineStateSet::new([
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]),
    )
}

fn evidence() -> NativeFuelTransferRuntimeEvidence {
    NativeFuelTransferRuntimeEvidence::new(
        x86_plan(),
        text(transfer_entry(), 512, vec![1, 2, 3, 4]),
        text(resume_entry(), 516, vec![5, 6, 7, 8]),
        footprint(MachineRegister::X86Rax),
        128,
    )
    .expect("complete transfer-runtime evidence")
}

#[test]
fn transfer_evidence_retains_exact_bytes_spans_and_resources() {
    let evidence = evidence();
    assert_eq!(evidence.plan(), &x86_plan());
    assert_eq!(evidence.transfer_text().span().text_offset, 512);
    assert_eq!(evidence.transfer_text().unrelocated_bytes(), &[1, 2, 3, 4]);
    assert_eq!(evidence.resume_text().final_bytes(), &[5, 6, 7, 8]);
    assert_eq!(evidence.sponsor_stack_peak_bytes(), 128);
    assert_ne!(evidence.fingerprint(), 0);
    assert_eq!(evidence.fingerprint(), evidence.report_fingerprint());
}

#[test]
fn every_evidence_mutation_changes_fingerprint() {
    let baseline = evidence();
    let build = |transfer, resume, footprint, peak| {
        NativeFuelTransferRuntimeEvidence::new(x86_plan(), transfer, resume, footprint, peak)
            .unwrap()
    };
    let variants = [
        build(
            NativeFuelRuntimeTextEvidence::new(
                transfer_entry(),
                NativeFuelRuntimeTextSpan {
                    text_offset: 512,
                    byte_count: 4,
                },
                vec![9, 2, 3, 4],
                vec![1, 2, 3, 4],
            )
            .unwrap(),
            text(resume_entry(), 516, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rax),
            128,
        ),
        build(
            NativeFuelRuntimeTextEvidence::new(
                transfer_entry(),
                NativeFuelRuntimeTextSpan {
                    text_offset: 512,
                    byte_count: 4,
                },
                vec![1, 2, 3, 4],
                vec![9, 2, 3, 4],
            )
            .unwrap(),
            text(resume_entry(), 516, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rax),
            128,
        ),
        build(
            text(transfer_entry(), 500, vec![1, 2, 3, 4]),
            text(resume_entry(), 516, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rax),
            128,
        ),
        build(
            text(transfer_entry(), 512, vec![1, 2, 3, 4]),
            text(resume_entry(), 516, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rcx),
            128,
        ),
        build(
            text(transfer_entry(), 512, vec![1, 2, 3, 4]),
            text(resume_entry(), 516, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rax),
            64,
        ),
    ];
    assert!(
        variants
            .iter()
            .all(|variant| variant.fingerprint() != baseline.fingerprint())
    );
}

#[test]
fn transfer_evidence_shape_mutations_reject() {
    assert_eq!(
        NativeFuelRuntimeTextEvidence::new(
            transfer_entry(),
            NativeFuelRuntimeTextSpan {
                text_offset: 0,
                byte_count: 2
            },
            vec![1],
            vec![1, 2],
        ),
        Err(NativeFuelTransferEvidenceError::InvalidTextSpan)
    );
    assert_eq!(
        NativeFuelRuntimeTextEvidence::new(
            transfer_entry(),
            NativeFuelRuntimeTextSpan {
                text_offset: usize::MAX,
                byte_count: 1
            },
            vec![1],
            vec![1],
        ),
        Err(NativeFuelTransferEvidenceError::InvalidTextSpan)
    );

    let plan = x86_plan();
    let transfer = text(transfer_entry(), 512, vec![1, 2, 3, 4]);
    let resume = text(resume_entry(), 516, vec![5, 6, 7, 8]);
    assert_eq!(
        NativeFuelTransferRuntimeEvidence::new(
            plan.clone(),
            text(resume_entry(), 512, vec![1, 2, 3, 4]),
            resume.clone(),
            footprint(MachineRegister::X86Rax),
            128,
        ),
        Err(NativeFuelTransferEvidenceError::TransferEntryMismatch)
    );
    assert_eq!(
        NativeFuelTransferRuntimeEvidence::new(
            plan.clone(),
            transfer.clone(),
            text(transfer_entry(), 516, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rax),
            128,
        ),
        Err(NativeFuelTransferEvidenceError::ResumeEntryMismatch)
    );
    assert_eq!(
        NativeFuelTransferRuntimeEvidence::new(
            plan.clone(),
            transfer.clone(),
            text(resume_entry(), 514, vec![5, 6, 7, 8]),
            footprint(MachineRegister::X86Rax),
            128,
        ),
        Err(NativeFuelTransferEvidenceError::OverlappingTextSpans)
    );
    let wrong_target =
        StateFootprintEvidence::new(RegisterSet::new([MachineRegister::Aarch64X(0)]), states());
    assert_eq!(
        NativeFuelTransferRuntimeEvidence::new(
            plan.clone(),
            transfer.clone(),
            resume.clone(),
            wrong_target,
            128,
        ),
        Err(NativeFuelTransferEvidenceError::FootprintTargetMismatch)
    );
    let incomplete = StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax]),
        MachineStateSet::empty(),
    );
    assert_eq!(
        NativeFuelTransferRuntimeEvidence::new(
            plan.clone(),
            transfer.clone(),
            resume.clone(),
            incomplete,
            128,
        ),
        Err(NativeFuelTransferEvidenceError::IncompleteStateFootprint)
    );
    for peak in [0, 257] {
        assert_eq!(
            NativeFuelTransferRuntimeEvidence::new(
                plan.clone(),
                transfer.clone(),
                resume.clone(),
                footprint(MachineRegister::X86Rax),
                peak,
            ),
            Err(NativeFuelTransferEvidenceError::StackPeakExceedsPlan)
        );
    }
}
