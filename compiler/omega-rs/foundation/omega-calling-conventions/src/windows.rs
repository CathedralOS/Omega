use crate::{
    HostAbiPlan, HostBinding, HostBindingMechanism, HostBoundaryPolicy, PlatformCallData,
    host_operation, insert_platform_lowering,
};

/// The Windows import catalog rows: (capability, operation, library, symbol).
/// Single source of truth for BOTH the host-ABI bindings and the PE import
/// table's symbol->library grouping.
pub const WINDOWS_IMPORT_ROWS: &[(&str, &str, &str, &str)] = &[
    ("Stdin", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
    ("Stdin", "read_file", "Kernel32.dll", "ReadFile"),
    ("Stdin", "read", "Kernel32.dll", "ReadFile"),
    ("Stdout", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
    ("Stdout", "write", "Kernel32.dll", "WriteFile"),
    ("Stdout", "write_file", "Kernel32.dll", "WriteFile"),
    ("Stderr", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
    ("Stderr", "write", "Kernel32.dll", "WriteFile"),
    ("Stderr", "write_file", "Kernel32.dll", "WriteFile"),
    ("Process", "exit_process", "Kernel32.dll", "ExitProcess"),
    ("Clock", "sleep", "Kernel32.dll", "Sleep"),
    ("Clock", "tick_count", "Kernel32.dll", "GetTickCount64"),
    ("Input", "key_state", "User32.dll", "GetAsyncKeyState"),
];

/// The DLL a Windows import symbol belongs to, per the catalog. `None` for
/// symbols outside the catalog (callers default to KERNEL32.dll, the
/// historical single-DLL behavior).
pub fn windows_import_library(symbol: &str) -> Option<&'static str> {
    WINDOWS_IMPORT_ROWS
        .iter()
        .find(|(_, _, _, row_symbol)| *row_symbol == symbol)
        .map(|(_, _, library, _)| *library)
}

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: "omega::host::targets::windows".into(),
        checked: true,
    });

    plan.bindings.insert_many(
        WINDOWS_IMPORT_ROWS
            .iter()
            .map(|(capability, operation, library, symbol)| {
                windows_import(capability, operation, library, symbol)
            }),
    );

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
        "tick_count",
        [host_operation("Clock", "tick_count")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "key_state",
        [host_operation("Input", "key_state")],
        PlatformCallData::None,
    );
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
