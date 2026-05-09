use crate::{
    HostAbiPlan, HostBinding, HostBindingMechanism, PlatformCallData, host_operation,
    insert_platform_lowering,
};

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    plan.bindings.insert_many([
        darwin_import("Stdin", "read", "_read"),
        darwin_import("Stdout", "write", "_write"),
        darwin_import("Process", "exit", "_exit"),
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

fn darwin_import(capability: &str, operation: &str, symbol: &str) -> HostBinding {
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Import {
            library: "libSystem.B.dylib".to_owned(),
            symbol: symbol.to_owned(),
        },
        trust_policy: "omega::host::targets::darwin".to_owned(),
    }
}
