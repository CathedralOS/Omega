use crate::{
    HostAbiPlan, HostBinding, HostBindingMechanism, PlatformCallData, host_operation,
    insert_platform_lowering,
};
use omega_target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinuxSyscallNumbers {
    read: u32,
    write: u32,
    exit_group: u32,
}

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    let syscall_numbers = linux_syscall_numbers(plan.target.architecture);
    plan.bindings.insert_many([
        linux_syscall("Stdin", "read", syscall_numbers.read),
        linux_syscall("Stdout", "write", syscall_numbers.write),
        linux_syscall("Process", "exit_group", syscall_numbers.exit_group),
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
        "read_line",
        [host_operation("Stdin", "read")],
        PlatformCallData::MutableOutputBuffer { byte_capacity: 256 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "exit_process",
        [host_operation("Process", "exit_group")],
        PlatformCallData::None,
    );
}

fn linux_syscall_numbers(architecture: Architecture) -> LinuxSyscallNumbers {
    match architecture {
        Architecture::Aarch64 => LinuxSyscallNumbers {
            read: 63,
            write: 64,
            exit_group: 94,
        },
        Architecture::X86_64 => LinuxSyscallNumbers {
            read: 0,
            write: 1,
            exit_group: 231,
        },
    }
}

fn linux_syscall(capability: &str, operation: &str, number: u32) -> HostBinding {
    HostBinding {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
        mechanism: HostBindingMechanism::Syscall {
            name: operation.to_owned(),
            number,
            number_register: 8,
            supervisor_call: 0,
        },
        trust_policy: "omega::host::targets::linux".to_owned(),
    }
}
