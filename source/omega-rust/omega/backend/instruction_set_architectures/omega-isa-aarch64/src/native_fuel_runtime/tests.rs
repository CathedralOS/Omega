use super::*;
use omega_installation_evidence::{
    NativeFuelActivationStateSlot, NativeFuelContextLayout, NativeFuelRuntimeEntryIdentity,
    NativeFuelSponsorStackPlan,
};

fn state_for(slots: &[NativeFuelActivationStateSlot]) -> MachineStateSet {
    let mut states = vec![MachineState::InstructionPointer, MachineState::StackPointer];
    for slot in slots {
        states.push(match slot.value {
            NativeFuelSavedValue::Register(MachineRegister::Aarch64V(_)) => {
                MachineState::VectorRegisters
            }
            NativeFuelSavedValue::Register(_) => MachineState::GeneralRegisters,
            NativeFuelSavedValue::Flags => MachineState::Flags,
            NativeFuelSavedValue::StackPointer => MachineState::StackPointer,
        });
    }
    MachineStateSet::new(states)
}

fn slots_at(base: u32) -> Vec<NativeFuelActivationStateSlot> {
    vec![
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::Aarch64X(0)),
            context_offset: base,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Flags,
            context_offset: base + 8,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::Aarch64V(0)),
            context_offset: base + 16,
            byte_count: 16,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::StackPointer,
            context_offset: base + 32,
            byte_count: 8,
        },
    ]
}

fn context(activation_state_offset: u32, byte_size: u32) -> NativeFuelContextLayout {
    NativeFuelContextLayout {
        byte_size,
        alignment: 16,
        remaining_units_offset: 0,
        unpaid_site_kind_offset: 8,
        unpaid_site_identity_offset: 16,
        required_units_offset: 24,
        transfer_entry_offset: 32,
        retry_code_offset_offset: 40,
        sponsor_stack_top_offset: 48,
        activation_state_offset,
        activation_state_byte_count: 40,
    }
}

fn plan_with(
    profile: TargetProfile,
    context: NativeFuelContextLayout,
    slots: Vec<NativeFuelActivationStateSlot>,
    sponsor_stack: NativeFuelSponsorStackPlan,
) -> NativeFuelTransferRuntimePlanProjection {
    let state = state_for(&slots);
    NativeFuelTransferRuntimePlanProjection::new(
        profile,
        profile.native_target(),
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(28),
        },
        context,
        slots,
        sponsor_stack,
        state,
        state,
        state,
        NativeFuelRuntimeEntryIdentity {
            section_identity: 1,
            symbol_identity: 2,
        },
        NativeFuelRuntimeEntryIdentity {
            section_identity: 1,
            symbol_identity: 3,
        },
    )
    .expect("structural AArch64 transfer plan")
}

fn plan() -> NativeFuelTransferRuntimePlanProjection {
    plan_with(
        TargetProfile::LinuxArm64,
        context(64, 112),
        slots_at(64),
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    )
}

#[test]
fn canonical_transfer_and_resume_bytes_are_exact() {
    let encoded = encode_native_fuel_transfer_runtime(&plan()).expect("Linux AArch64 runtime");
    let words = |bytes: &[u8]| {
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        words(encoded.transfer_bytes()),
        vec![
            0xf900_2380, // str x0, [x28, #64]
            0xd53b_4210, // mrs x16, nzcv
            0xf900_2790, // str x16, [x28, #72]
            0x3d80_1780, // str q0, [x28, #80]
            0x9100_03f1, // mov x17, sp
            0xf900_3391, // str x17, [x28, #96]
            0xf940_1b91, // ldr x17, [x28, #48]
            0x9100_023f, // mov sp, x17
            0xd100_43ff, // sub sp, sp, #16
            0xf900_03fc, // str x28, [sp]
            0x9400_0000, // bl sponsor
            0xf940_03fc, // ldr x28, [sp]
            0x9100_43ff, // add sp, sp, #16
        ]
    );
    assert_eq!(encoded.sponsor_call_branch26_offset(), 40);
    assert_eq!(
        &encoded.transfer_bytes()[40..44],
        &0x9400_0000_u32.to_le_bytes()
    );
    assert_eq!(
        words(encoded.resume_bytes()),
        vec![
            0x9000_0010, // adrp x16, text base
            0x9100_0210, // add x16, x16, :lo12:text base
            0xf940_1791, // ldr x17, [x28, #40]
            0x8b11_0210, // add x16, x16, x17
            0xf940_2380, // ldr x0, [x28, #64]
            0x3dc0_1780, // ldr q0, [x28, #80]
            0xf940_2791, // ldr x17, [x28, #72]
            0xd51b_4211, // msr nzcv, x17
            0xf940_3391, // ldr x17, [x28, #96]
            0x9100_023f, // mov sp, x17
            0xd61f_0200, // br x16
        ]
    );
    assert_eq!(encoded.retry_text_page21_offset(), 0);
    assert_eq!(encoded.retry_text_page_offset12_offset(), 4);
}

#[test]
fn runtime_reports_exact_physical_resources() {
    let encoded = encode_native_fuel_transfer_runtime(&plan()).unwrap();
    assert_eq!(encoded.realized_sponsor_stack_peak_bytes(), 16);
    assert_eq!(
        encoded.physical_state_footprint().registers().as_slice(),
        &[
            MachineRegister::Aarch64X(0),
            MachineRegister::Aarch64X(16),
            MachineRegister::Aarch64X(17),
            MachineRegister::Aarch64X(28),
            MachineRegister::Aarch64X(30),
            MachineRegister::Aarch64V(0),
        ]
    );
    assert_eq!(
        encoded.physical_state_footprint().machine_state(),
        MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ])
    );
}

#[test]
fn high_register_encodings_remain_canonical() {
    let slots = vec![
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::Aarch64V(31)),
            context_offset: 64,
            byte_count: 16,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::Aarch64X(30)),
            context_offset: 80,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Flags,
            context_offset: 88,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::StackPointer,
            context_offset: 96,
            byte_count: 8,
        },
    ];
    let encoded = encode_native_fuel_transfer_runtime(&plan_with(
        TargetProfile::LinuxArm64,
        context(64, 112),
        slots,
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    ))
    .unwrap();
    assert_eq!(
        &encoded.transfer_bytes()[0..4],
        &0x3d80_139f_u32.to_le_bytes()
    );
    assert_eq!(
        &encoded.transfer_bytes()[4..8],
        &0xf900_2b9e_u32.to_le_bytes()
    );
    assert!(
        encoded
            .resume_bytes()
            .windows(4)
            .any(|bytes| bytes == 0x3dc0_139f_u32.to_le_bytes())
    );
    assert!(
        encoded
            .resume_bytes()
            .windows(4)
            .any(|bytes| bytes == 0xf940_2b9e_u32.to_le_bytes())
    );
}

#[test]
fn wrong_profile_reserved_registers_offsets_and_stack_reject() {
    let darwin = plan_with(
        TargetProfile::MacosArm64,
        context(64, 112),
        slots_at(64),
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    );
    assert!(encode_native_fuel_transfer_runtime(&darwin).is_err());

    for register in [
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(28),
    ] {
        let mut slots = slots_at(64);
        slots[0].value = NativeFuelSavedValue::Register(register);
        assert!(
            encode_native_fuel_transfer_runtime(&plan_with(
                TargetProfile::LinuxArm64,
                context(64, 112),
                slots,
                NativeFuelSponsorStackPlan {
                    alignment: 16,
                    byte_ceiling: 256,
                },
            ))
            .is_err()
        );
    }

    let large_base = 65_536;
    assert!(
        encode_native_fuel_transfer_runtime(&plan_with(
            TargetProfile::LinuxArm64,
            context(large_base, large_base + 48),
            slots_at(large_base),
            NativeFuelSponsorStackPlan {
                alignment: 16,
                byte_ceiling: 256,
            },
        ))
        .is_err()
    );
    assert!(
        encode_native_fuel_transfer_runtime(&plan_with(
            TargetProfile::LinuxArm64,
            context(64, 112),
            slots_at(64),
            NativeFuelSponsorStackPlan {
                alignment: 8,
                byte_ceiling: 256,
            },
        ))
        .is_err()
    );
}
