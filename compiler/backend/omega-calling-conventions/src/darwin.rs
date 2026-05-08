use crate::{
    HostAbiPlan, HostBinding, HostBindingMechanism, PlatformCallData, host_operation,
    insert_platform_lowering,
};

const DARWIN_SYSCALL_CLASS_UNIX: u32 = 0x0200_0000;

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    plan.bindings.insert_many([
        darwin_syscall("Stdin", "read", DARWIN_SYSCALL_CLASS_UNIX | 3),
        darwin_syscall("Stdout", "write", DARWIN_SYSCALL_CLASS_UNIX | 4),
        darwin_syscall("Process", "exit", DARWIN_SYSCALL_CLASS_UNIX | 1),
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
        [host_operation("Process", "exit")],
        PlatformCallData::None,
    );
}

fn darwin_syscall(capability: &str, operation: &str, number: u32) -> HostBinding {
    HostBinding {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
        mechanism: HostBindingMechanism::Syscall {
            name: operation.to_owned(),
            number,
            number_register: 16,
            supervisor_call: 0x80,
        },
        trust_policy: "omega::host::targets::darwin".to_owned(),
    }
}
