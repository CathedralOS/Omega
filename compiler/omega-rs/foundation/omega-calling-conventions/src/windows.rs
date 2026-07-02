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
        // Runtime text-read lowering tags the `ReadRuntimeTextLine` instruction with
        // the portable `Stdin.read` operation key (see omega-target-operations'
        // abstract_conversions), mirroring the single-call Linux/Darwin shape. The
        // Windows line read is a GetStdHandle + ReadFile sequence emitted inline, so
        // bind the portable `Stdin.read` key to the same kernel32 `ReadFile` import
        // that the explicit `Stdin.read_file` operation uses. Without this the
        // emission planner fails the read-line binding lookup with "missing host
        // binding for runtime text read operation Stdin.read".
        windows_import("Stdin", "read", "Kernel32.dll", "ReadFile"),
        windows_import("Stdout", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
        windows_import("Stdout", "write", "Kernel32.dll", "WriteFile"),
        windows_import("Stdout", "write_file", "Kernel32.dll", "WriteFile"),
        windows_import("Stderr", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
        windows_import("Stderr", "write", "Kernel32.dll", "WriteFile"),
        windows_import("Stderr", "write_file", "Kernel32.dll", "WriteFile"),
        windows_import("Process", "exit_process", "Kernel32.dll", "ExitProcess"),
        windows_import("Clock", "sleep", "Kernel32.dll", "Sleep"),
        windows_import("Clock", "tick_count", "Kernel32.dll", "GetTickCount64"),
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
        "write_error_line",
        [
            host_operation("Stderr", "get_std_handle"),
            host_operation("Stderr", "write_file"),
        ],
        PlatformCallData::FirstTextArgument {
            append_newline: true,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_error",
        [
            host_operation("Stderr", "get_std_handle"),
            host_operation("Stderr", "write_file"),
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
    // Frame pacing for timed loops: `Sleep(DWORD ms)` -- a single kernel32 call,
    // one u32 arg in ecx, no return (non-terminal). Same call shape as
    // `get_std_handle`/`exit_process`.
    insert_platform_lowering(
        plan,
        "*",
        "sleep",
        [host_operation("Clock", "sleep")],
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
