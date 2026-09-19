use super::integer_signature;
use crate::plans::{
    CallingPolicy, EntryControl, MachineRegister, RegisterSet, evaluate_call_plan,
    validate_call_plan,
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
    ] {
        assert_eq!(CallingPolicy::native_for_target(target), policy);
    }
}

#[test]
fn native_for_target_resolves_a_policy_for_every_profile_target() {
    for profile in target::TargetProfile::ALL {
        let Some(native) = profile.native_realization() else {
            continue;
        };
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
        let Some(native) = profile.native_realization() else {
            continue;
        };
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
