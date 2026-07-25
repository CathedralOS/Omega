use crate::{
    CallSignature, CallingPolicy, HostAbiPlan, HostBinding, HostBindingMechanism,
    HostBoundaryPolicy, PlatformCallData, ValueShape, evaluate_ordinary_boundary_entry_plan,
    host_operation, insert_platform_lowering,
};
use omega_target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinuxSyscallNumbers {
    read: u32,
    write: u32,
    exit_group: u32,
    clock_gettime: u32,
    nanosleep: u32,
}

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    let policy: std::sync::Arc<str> = "omega::host::targets::linux".into();
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: std::sync::Arc::clone(&policy),
        checked: true,
    });

    let syscall_numbers = linux_syscall_numbers(plan.target.architecture);
    plan.bindings.insert_many([
        linux_syscall("Stdin", "read", syscall_numbers.read, &policy),
        linux_syscall("Stdout", "write", syscall_numbers.write, &policy),
        linux_syscall("Stderr", "write", syscall_numbers.write, &policy),
        linux_syscall("Process", "exit_group", syscall_numbers.exit_group, &policy),
        linux_clock_gettime_syscall(
            "monotonic_ticks",
            syscall_numbers.clock_gettime,
            &policy,
            plan.target.architecture,
        ),
        linux_clock_gettime_syscall(
            "wall_clock_raw",
            syscall_numbers.clock_gettime,
            &policy,
            plan.target.architecture,
        ),
        linux_timespec_syscall(
            "sleep",
            "nanosleep",
            syscall_numbers.nanosleep,
            &policy,
            plan.target.architecture,
        ),
    ]);

    insert_platform_lowering(
        plan,
        "*",
        "write_line",
        [host_operation("Stdout", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: true,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write",
        [host_operation("Stdout", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: false,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_error_line",
        [host_operation("Stderr", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: true,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_error",
        [host_operation("Stderr", "write")],
        PlatformCallData::FirstTextArgument {
            append_newline: false,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "read_line",
        [host_operation("Stdin", "read")],
        PlatformCallData::MutableOutputBuffer { byte_capacity: 256 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "read_byte",
        [host_operation("Stdin", "read")],
        PlatformCallData::SingleByteRead,
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_byte",
        [host_operation("Stdout", "write")],
        PlatformCallData::SingleByteWrite,
    );
    insert_platform_lowering(
        plan,
        "*",
        "exit_process",
        [host_operation("Process", "exit_group")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks",
        [host_operation("Clock", "monotonic_ticks")],
        PlatformCallData::TimespecResult { clock_id: 1 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "tick_count",
        [host_operation("Clock", "monotonic_ticks")],
        PlatformCallData::TimespecResult { clock_id: 1 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks_per_second",
        [host_operation("Clock", "monotonic_ticks_per_second")],
        PlatformCallData::ConstantResult {
            value: 1_000_000_000,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_raw",
        [host_operation("Clock", "wall_clock_raw")],
        PlatformCallData::TimespecResult { clock_id: 0 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_units_per_second",
        [host_operation("Clock", "wall_clock_units_per_second")],
        PlatformCallData::ConstantResult {
            value: 1_000_000_000,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_epoch_offset_seconds",
        [host_operation("Clock", "wall_clock_epoch_offset_seconds")],
        PlatformCallData::ConstantResult { value: 0 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "sleep",
        [host_operation("Clock", "sleep")],
        PlatformCallData::TimespecArgument,
    );
}

fn linux_syscall_numbers(architecture: Architecture) -> LinuxSyscallNumbers {
    match architecture {
        Architecture::Aarch64 => LinuxSyscallNumbers {
            read: 63,
            write: 64,
            exit_group: 94,
            clock_gettime: 113,
            nanosleep: 101,
        },
        Architecture::X86_64 => LinuxSyscallNumbers {
            read: 0,
            write: 1,
            exit_group: 231,
            clock_gettime: 228,
            nanosleep: 35,
        },
    }
}

pub fn linux_clock_gettime_syscall_number(architecture: Architecture) -> u32 {
    linux_syscall_numbers(architecture).clock_gettime
}

pub fn linux_nanosleep_syscall_number(architecture: Architecture) -> u32 {
    linux_syscall_numbers(architecture).nanosleep
}

fn linux_clock_gettime_syscall(
    operation: &str,
    number: u32,
    policy: &std::sync::Arc<str>,
    architecture: Architecture,
) -> HostBinding {
    let calling_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        // clockid_t is widened into the syscall word; the second word is the
        // compiler-owned address of the temporary `timespec`.
        parameters: vec![word, word],
        result: Some(word),
    };
    let boundary_entry_plan = evaluate_ordinary_boundary_entry_plan(calling_policy, &signature)
        .expect("the built-in Linux clock_gettime signature must have a syscall plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names("Clock", operation),
        mechanism: HostBindingMechanism::Syscall {
            name: "clock_gettime".into(),
            number,
            number_register: 8,
            supervisor_call: 0,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan: Some(boundary_entry_plan),
    }
}

fn linux_timespec_syscall(
    operation: &str,
    name: &str,
    number: u32,
    policy: &std::sync::Arc<str>,
    architecture: Architecture,
) -> HostBinding {
    let calling_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![word, word],
        result: Some(word),
    };
    let boundary_entry_plan = evaluate_ordinary_boundary_entry_plan(calling_policy, &signature)
        .expect("the built-in Linux timespec syscall signature must have a syscall plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names("Clock", operation),
        mechanism: HostBindingMechanism::Syscall {
            name: name.into(),
            number,
            number_register: 8,
            supervisor_call: 0,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan: Some(boundary_entry_plan),
    }
}

fn linux_syscall(
    capability: &str,
    operation: &str,
    number: u32,
    policy: &std::sync::Arc<str>,
) -> HostBinding {
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Syscall {
            name: operation.into(),
            number,
            number_register: 8,
            supervisor_call: 0,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan: None,
    }
}
