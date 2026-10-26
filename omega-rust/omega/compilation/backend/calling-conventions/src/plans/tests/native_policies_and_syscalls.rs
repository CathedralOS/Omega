use super::integer_signature;
use crate::plans::{
    CallSignature, CallingPolicy, EntryControl, MachineRegister, RegisterSet, ValueLocation,
    ValueShape, evaluate_call_plan, validate_call_plan,
};
use target::Architecture;

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
        (
            target::NativeTarget::macos_x64(),
            CallingPolicy::SystemVAMD64,
        ),
    ] {
        assert_eq!(CallingPolicy::native_for_target(target), policy);
        assert_eq!(policy.architecture(), target.architecture);
    }
}

#[test]
fn native_for_target_fails_closed_for_undeclared_pairs() {
    let undeclared = target::NativeTarget {
        architecture: Architecture::Aarch64,
        object_format: target::ObjectFormat::Coff,
        pointer_size: 8,
        pointer_alignment: 8,
    };
    assert!(
        std::panic::catch_unwind(|| CallingPolicy::native_for_target(undeclared)).is_err(),
        "undeclared pair must fail closed"
    );
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
        (target::NativeTarget::macos_x64(), None),
    ] {
        assert_eq!(CallingPolicy::native_syscall_for_target(target), policy);
        if let Some(policy) = policy {
            assert_eq!(policy.architecture(), target.architecture);
        }
    }
}

#[test]
fn native_syscall_for_target_fails_closed_for_undeclared_pairs() {
    let undeclared = target::NativeTarget {
        architecture: Architecture::Aarch64,
        object_format: target::ObjectFormat::Coff,
        pointer_size: 8,
        pointer_alignment: 8,
    };
    assert!(
        std::panic::catch_unwind(|| { CallingPolicy::native_syscall_for_target(undeclared) })
            .is_err(),
        "undeclared pair must fail closed"
    );
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
fn syscall_two_word_formals_use_physical_slot_capacity() {
    for (policy, registers) in [
        (
            CallingPolicy::LinuxSyscallX86_64,
            vec![
                MachineRegister::X86Rdi,
                MachineRegister::X86Rsi,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
            ],
        ),
        (
            CallingPolicy::LinuxSyscallAarch64,
            (0..6).map(MachineRegister::Aarch64X).collect(),
        ),
    ] {
        // A descriptor remains one formal; a scalar after it uses the next
        // physical slot, not the next formal ordinal. Empty values use no slot.
        let mut signature = CallSignature {
            parameters: vec![
                ValueShape::integer(4, 4),
                ValueShape::integer(0, 1),
                ValueShape::integer(16, 8),
                ValueShape::integer(8, 8),
                ValueShape::integer(16, 8),
            ],
            result: Some(ValueShape::integer(8, 8)),
        };
        let plan = evaluate_call_plan(policy, &signature).expect("six-word syscall plan");
        assert_eq!(plan.parameters.len(), 5);
        assert!(plan.parameters[1].locations.is_empty());
        let expected: Vec<_> = registers
            .iter()
            .copied()
            .zip([4, 8, 8, 8, 8, 8])
            .zip([0, 0, 8, 0, 0, 8])
            .map(|((register, byte_size), value_byte_offset)| {
                ValueLocation::register(register, value_byte_offset, byte_size)
            })
            .collect();
        assert_eq!(
            plan.parameters
                .iter()
                .flat_map(|parameter| parameter.locations.iter().cloned())
                .collect::<Vec<_>>(),
            expected
        );
        validate_call_plan(&plan, &signature).expect("fragmented formals validate");

        signature.parameters.push(ValueShape::integer(8, 8));
        assert!(
            evaluate_call_plan(policy, &signature).is_err(),
            "seven physical words reject"
        );
        signature.parameters.pop();
        signature.result = Some(ValueShape::integer(16, 8));
        assert!(
            evaluate_call_plan(policy, &signature).is_err(),
            "result remains one word"
        );
        signature.result = None;
        signature.parameters[2] = ValueShape::integer(17, 8);
        assert!(
            evaluate_call_plan(policy, &signature).is_err(),
            "three-word formal makes this signature exceed six words"
        );
        signature.parameters[2] = ValueShape::float(8);
        assert!(
            evaluate_call_plan(policy, &signature).is_err(),
            "floating formal rejects"
        );

        let three_words = CallSignature {
            parameters: vec![ValueShape::integer(17, 8)],
            result: None,
        };
        let plan = evaluate_call_plan(policy, &three_words).expect("three physical words fit");
        assert_eq!(
            plan.parameters[0].locations,
            vec![
                ValueLocation::register(registers[0], 0, 8),
                ValueLocation::register(registers[1], 8, 8),
                ValueLocation::register(registers[2], 16, 1),
            ]
        );
        validate_call_plan(&plan, &three_words).expect("partial final word retains exact coverage");
        let seven_words = CallSignature {
            parameters: vec![ValueShape::integer(49, 8)],
            result: None,
        };
        assert!(
            evaluate_call_plan(policy, &seven_words).is_err(),
            "one formal cannot consume seven words"
        );
    }
}
