use crate::{
    HostAbiPlan, HostBinding, HostBindingMechanism, HostBoundaryPolicy, PlatformCallData,
    host_operation, insert_platform_lowering,
};

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: "omega::host::targets::windows".into(),
        checked: true,
    });

    plan.bindings.insert_many([
        windows_import("Stdin", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
        windows_import("Stdin", "read_file", "Kernel32.dll", "ReadFile"),
        windows_import("Stdout", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
        windows_import("Stdout", "write", "Kernel32.dll", "WriteFile"),
        windows_import("Stdout", "write_file", "Kernel32.dll", "WriteFile"),
        windows_import("Process", "exit_process", "Kernel32.dll", "ExitProcess"),
    ]);

    insert_platform_lowering(
        plan,
        "*",
        "write_line",
        [
            host_operation("Stdout", "get_std_handle"),
            host_operation("Stdout", "write_file"),
        ],
        PlatformCallData::FirstTextArgument {
            append_newline: true,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write",
        [
            host_operation("Stdout", "get_std_handle"),
            host_operation("Stdout", "write_file"),
        ],
        PlatformCallData::FirstTextArgument {
            append_newline: false,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "read_line",
        [
            host_operation("Stdin", "get_std_handle"),
            host_operation("Stdin", "read_file"),
        ],
        PlatformCallData::MutableOutputBuffer { byte_capacity: 256 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "exit_process",
        [host_operation("Process", "exit_process")],
        PlatformCallData::None,
    );
}

fn windows_import(capability: &str, operation: &str, library: &str, symbol: &str) -> HostBinding {
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Import {
            library: library.into(),
            symbol: symbol.into(),
        },
        boundary_policy: "omega::host::targets::windows".into(),
    }
}
