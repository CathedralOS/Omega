//! Exact native-fuel transfer plans shared by ranked publication fixtures.

use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet};
use omega_installation_evidence::{
    NativeFuelActivationStateSlot, NativeFuelContextLayout, NativeFuelRuntimeEntryIdentity,
    NativeFuelSavedValue, NativeFuelSponsorStackPlan, NativeFuelTargetPlanProjection,
    NativeFuelTransferRuntimePlanProjection, SponsorContextTransport,
};
use omega_target::{NativeTarget, TargetProfile};

pub(super) fn transfer_runtime_plan(
    target: NativeTarget,
) -> NativeFuelTransferRuntimePlanProjection {
    if target == NativeTarget::linux_x64() {
        x86_64_transfer_runtime_plan()
    } else if target == NativeTarget::linux_arm64() {
        aarch64_transfer_runtime_plan()
    } else {
        panic!("ranked native-fuel publication fixture has no transfer plan for {target:?}")
    }
}

pub(super) fn target_policy(
    plan: &NativeFuelTransferRuntimePlanProjection,
) -> NativeFuelTargetPlanProjection {
    NativeFuelTargetPlanProjection {
        profile: plan.profile(),
        target: plan.target(),
        transport: plan.transport(),
        context: plan.context(),
        transfer_plan_identity: plan.normalized_identity(),
    }
}

pub(super) fn assert_ranked_publication_round_trips(
    validated: &omega_image_emission::ValidatedNativeFuelArtifact,
    metered: &omega_machine_code::NativeFuelInstrumentedFunction,
    expected_rebase: omega_machine_code::NativeFuelRankedU32CountdownRebaseRecord,
    canonical: psi_terminal_codec::CanonicalTerminalArtifact,
) {
    let metered_image = omega_image_emission::emit_native_fuel_executable_image(validated, 0)
        .expect("ranked native-fuel direct image replays final publication custody");
    assert_eq!(
        metered_image.functions()[0].ranked_u32_countdown,
        Some(expected_rebase)
    );
    assert_eq!(metered_image.metered_text_bytes(), validated.text_bytes());
    assert_eq!(
        metered_image.output().final_text_bytes,
        validated.text_bytes()
    );
    assert_eq!(metered_image.output().final_image_relocations, 0);

    let charge_evidence = metered_image.charges();
    assert_eq!(charge_evidence.len(), 9);
    for (evidence, charge) in charge_evidence.iter().zip(&metered.charges) {
        assert_eq!(evidence.attribution.machine, metered.machine);
        assert_eq!(
            evidence.charge_text_offset,
            validated.functions()[0].text_offset + charge.charge_code_offset
        );
        assert_eq!(evidence.charge_byte_count, charge.charge_byte_count);
        assert_eq!(
            evidence.semantic_text_offset,
            validated.functions()[0].text_offset + charge.semantic_code_offset
        );
        assert_eq!(
            evidence.cold_dispatch_text_offset,
            validated.functions()[0].text_offset + charge.cold_dispatch_code_offset
        );
        assert_eq!(
            evidence.cold_dispatch_byte_count,
            charge.cold_dispatch_byte_count
        );
    }

    let metered_installation = omega_image_emission::build_native_fuel_installation_record(
        &metered_image,
        psi_core::ProfileDecisionId::new(2).expect("metered profile decision"),
    )
    .expect("ranked native-fuel direct image enters installation custody");
    assert!(metered_installation.functions()[0].ranked_u32_countdown);
    assert_eq!(
        metered_installation
            .native_fuel()
            .expect("direct native-fuel installation section")
            .charges(),
        charge_evidence
    );
    let metered_installation_bytes =
        omega_image_emission::encode_installation_record(&metered_installation)
            .expect("encode ranked native-fuel direct installation");
    let decoded_metered_installation =
        omega_image_emission::decode_installation_record(&metered_installation_bytes)
            .expect("decode ranked native-fuel direct installation");
    assert_eq!(decoded_metered_installation, metered_installation);
    omega_image_emission::validate_native_fuel_installation_record(
        &decoded_metered_installation,
        &metered_image,
    )
    .expect("decoded ranked native-fuel direct installation rejoins its exact image");

    let native = omega_native_artifact::RankedNativeFuelArtifact::from_replayed_parts(
        omega_native_artifact::RankedNativeFuelArtifactParts {
            psi_artifact: canonical,
            image: metered_image,
            installation: decoded_metered_installation,
        },
    )
    .expect("ranked native-fuel image enters source-free native-artifact custody");
    native
        .validate()
        .expect("ranked native-fuel artifact replays independently");
}

fn x86_64_transfer_runtime_plan() -> NativeFuelTransferRuntimePlanProjection {
    let state = MachineStateSet::new([
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::GeneralRegisters,
        MachineState::Flags,
    ]);
    NativeFuelTransferRuntimePlanProjection::new(
        TargetProfile::LinuxX64,
        NativeTarget::linux_x64(),
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::X86Rbx,
        },
        NativeFuelContextLayout {
            byte_size: 96,
            alignment: 16,
            remaining_units_offset: 0,
            unpaid_site_kind_offset: 8,
            unpaid_site_identity_offset: 16,
            required_units_offset: 24,
            transfer_entry_offset: 32,
            retry_code_offset_offset: 40,
            sponsor_stack_top_offset: 48,
            activation_state_offset: 64,
            activation_state_byte_count: 24,
        },
        vec![
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Register(MachineRegister::X86Rdi),
                context_offset: 64,
                byte_count: 8,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Flags,
                context_offset: 72,
                byte_count: 8,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::StackPointer,
                context_offset: 80,
                byte_count: 8,
            },
        ],
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
        state,
        state,
        state,
        NativeFuelRuntimeEntryIdentity {
            section_identity: 11,
            symbol_identity: 12,
        },
        NativeFuelRuntimeEntryIdentity {
            section_identity: 11,
            symbol_identity: 13,
        },
    )
    .expect("exact Linux x86-64 native-fuel transfer plan")
}

fn aarch64_transfer_runtime_plan() -> NativeFuelTransferRuntimePlanProjection {
    let state = MachineStateSet::new([
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::GeneralRegisters,
        MachineState::VectorRegisters,
        MachineState::Flags,
    ]);
    NativeFuelTransferRuntimePlanProjection::new(
        TargetProfile::LinuxArm64,
        NativeTarget::linux_arm64(),
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(28),
        },
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
        },
        vec![
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Register(MachineRegister::Aarch64X(0)),
                context_offset: 64,
                byte_count: 8,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Flags,
                context_offset: 72,
                byte_count: 8,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::Register(MachineRegister::Aarch64V(0)),
                context_offset: 80,
                byte_count: 16,
            },
            NativeFuelActivationStateSlot {
                value: NativeFuelSavedValue::StackPointer,
                context_offset: 96,
                byte_count: 8,
            },
        ],
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
        state,
        state,
        state,
        NativeFuelRuntimeEntryIdentity {
            section_identity: 21,
            symbol_identity: 22,
        },
        NativeFuelRuntimeEntryIdentity {
            section_identity: 21,
            symbol_identity: 23,
        },
    )
    .expect("exact Linux AArch64 native-fuel transfer plan")
}
