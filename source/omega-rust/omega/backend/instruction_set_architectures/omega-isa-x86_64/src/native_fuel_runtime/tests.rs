use super::*;
use omega_installation_evidence::{
    NativeFuelActivationStateSlot, NativeFuelContextLayout, NativeFuelRuntimeEntryIdentity,
    NativeFuelSponsorStackPlan,
};

fn state_for(slots: &[NativeFuelActivationStateSlot]) -> MachineStateSet {
    let mut states = vec![MachineState::InstructionPointer, MachineState::StackPointer];
    for slot in slots {
        states.push(match slot.value {
            NativeFuelSavedValue::Register(MachineRegister::X86Xmm(_))
            | NativeFuelSavedValue::Register(MachineRegister::Aarch64V(_)) => {
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
            value: NativeFuelSavedValue::Register(MachineRegister::X86Rax),
            context_offset: base,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Flags,
            context_offset: base + 8,
            byte_count: 8,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::X86Xmm(0)),
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
            register: MachineRegister::X86Rbx,
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
    .expect("structural x86-64 transfer plan")
}

fn plan() -> NativeFuelTransferRuntimePlanProjection {
    plan_with(
        TargetProfile::LinuxX64,
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
    let encoded = encode_native_fuel_transfer_runtime(&plan()).expect("Linux x86-64 runtime");
    assert_eq!(
        encoded.transfer_bytes(),
        &[
            0x48, 0x89, 0x83, 0x40, 0x00, 0x00, 0x00, // mov [rbx+64], rax
            0xf3, 0x0f, 0x7f, 0x83, 0x50, 0x00, 0x00, 0x00, // movdqu [rbx+80], xmm0
            0x48, 0x89, 0xa3, 0x60, 0x00, 0x00, 0x00, // mov [rbx+96], rsp
            0x48, 0x8b, 0xa3, 0x30, 0x00, 0x00, 0x00, // mov rsp, [rbx+48]
            0x9c, // pushfq on sponsor stack
            0x8f, 0x83, 0x48, 0x00, 0x00, 0x00, // pop [rbx+72]
            0x48, 0x83, 0xec, 0x10, // sub rsp, 16
            0x48, 0x89, 0x1c, 0x24, // mov [rsp], rbx
            0xe8, 0x00, 0x00, 0x00, 0x00, // call sponsor rel32
            0x48, 0x8b, 0x1c, 0x24, // mov rbx, [rsp]
            0x48, 0x83, 0xc4, 0x10, // add rsp, 16
        ]
    );
    assert_eq!(encoded.sponsor_call_rel32_field_offset(), 45);
    assert_eq!(
        &encoded.transfer_bytes()[encoded.sponsor_call_rel32_field_offset() - 1
            ..encoded.sponsor_call_rel32_field_offset() + 4],
        &[0xe8, 0, 0, 0, 0]
    );
    assert_eq!(
        encoded.resume_bytes(),
        &[
            0x4c, 0x8d, 0x15, 0x00, 0x00, 0x00, 0x00, // lea r10, [rip+text base]
            0x4c, 0x03, 0x93, 0x28, 0x00, 0x00, 0x00, // add r10, [rbx+40]
            0x4c, 0x8b, 0x9b, 0x60, 0x00, 0x00, 0x00, // mov r11, [rbx+96]
            0x48, 0x8b, 0x83, 0x40, 0x00, 0x00, 0x00, // mov rax, [rbx+64]
            0xf3, 0x0f, 0x6f, 0x83, 0x50, 0x00, 0x00, 0x00, // movdqu xmm0, [rbx+80]
            0xff, 0xb3, 0x48, 0x00, 0x00, 0x00, // push [rbx+72]
            0x9d, // popfq
            0x4c, 0x89, 0xdc, // mov rsp, r11
            0x41, 0xff, 0xe2, // jmp r10
        ]
    );
    assert_eq!(encoded.retry_text_base_rel32_field_offset(), 3);
    assert_eq!(
        &encoded.resume_bytes()[..7],
        &[0x4c, 0x8d, 0x15, 0, 0, 0, 0]
    );
}

#[test]
fn runtime_reports_exact_physical_resources() {
    let encoded = encode_native_fuel_transfer_runtime(&plan()).unwrap();
    assert_eq!(encoded.realized_sponsor_stack_peak_bytes(), 24);
    assert_eq!(
        encoded.physical_state_footprint().registers().as_slice(),
        &[
            MachineRegister::X86Rax,
            MachineRegister::X86Rbx,
            MachineRegister::X86Rsp,
            MachineRegister::X86R10,
            MachineRegister::X86R11,
            MachineRegister::X86Xmm(0),
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
fn high_x86_register_encodings_remain_canonical() {
    let slots = vec![
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::X86Xmm(9)),
            context_offset: 64,
            byte_count: 16,
        },
        NativeFuelActivationStateSlot {
            value: NativeFuelSavedValue::Register(MachineRegister::X86R12),
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
        TargetProfile::LinuxX64,
        context(64, 112),
        slots,
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    ))
    .unwrap();
    assert!(
        encoded
            .transfer_bytes()
            .starts_with(&[0xf3, 0x44, 0x0f, 0x7f, 0x8b, 0x40, 0, 0, 0])
    );
    assert_eq!(
        &encoded.transfer_bytes()[9..16],
        &[0x4c, 0x89, 0xa3, 0x50, 0, 0, 0]
    );
    assert!(
        encoded
            .resume_bytes()
            .windows(9)
            .any(|bytes| bytes == [0xf3, 0x44, 0x0f, 0x6f, 0x8b, 0x40, 0, 0, 0])
    );
}

#[test]
fn non_linux_x86_and_foreign_target_plans_reject() {
    let windows = plan_with(
        TargetProfile::WindowsX64,
        context(64, 112),
        slots_at(64),
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    );
    assert!(encode_native_fuel_transfer_runtime(&windows).is_err());

    let arm_slots = vec![
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
    ];
    let arm_state = state_for(&arm_slots);
    let arm = NativeFuelTransferRuntimePlanProjection::new(
        TargetProfile::LinuxArm64,
        TargetProfile::LinuxArm64.native_target(),
        SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::Aarch64X(28),
        },
        context(64, 112),
        arm_slots,
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
        arm_state,
        arm_state,
        arm_state,
        NativeFuelRuntimeEntryIdentity {
            section_identity: 1,
            symbol_identity: 2,
        },
        NativeFuelRuntimeEntryIdentity {
            section_identity: 1,
            symbol_identity: 3,
        },
    )
    .unwrap();
    assert!(encode_native_fuel_transfer_runtime(&arm).is_err());
}

#[test]
fn reserved_and_unencodable_saved_registers_reject() {
    for register in [
        MachineRegister::X86Rbx,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ] {
        let mut slots = slots_at(64);
        slots[0].value = NativeFuelSavedValue::Register(register);
        let plan = plan_with(
            TargetProfile::LinuxX64,
            context(64, 112),
            slots,
            NativeFuelSponsorStackPlan {
                alignment: 16,
                byte_ceiling: 256,
            },
        );
        assert!(encode_native_fuel_transfer_runtime(&plan).is_err());
    }

    let mut slots = slots_at(64);
    slots[2].value = NativeFuelSavedValue::Register(MachineRegister::X86Xmm(16));
    let plan = plan_with(
        TargetProfile::LinuxX64,
        context(64, 112),
        slots,
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    );
    assert!(encode_native_fuel_transfer_runtime(&plan).is_err());
}

#[test]
fn displacement_alignment_and_stack_ceiling_fail_closed() {
    let large_base = 0x8000_0000;
    let large = plan_with(
        TargetProfile::LinuxX64,
        context(large_base, 0x8000_0030),
        slots_at(large_base),
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 256,
        },
    );
    assert!(encode_native_fuel_transfer_runtime(&large).is_err());

    let weak_alignment = plan_with(
        TargetProfile::LinuxX64,
        context(64, 112),
        slots_at(64),
        NativeFuelSponsorStackPlan {
            alignment: 8,
            byte_ceiling: 256,
        },
    );
    assert!(encode_native_fuel_transfer_runtime(&weak_alignment).is_err());

    let short_stack = plan_with(
        TargetProfile::LinuxX64,
        context(64, 112),
        slots_at(64),
        NativeFuelSponsorStackPlan {
            alignment: 16,
            byte_ceiling: 16,
        },
    );
    assert!(encode_native_fuel_transfer_runtime(&short_stack).is_err());
}
