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
    ("Gui", "dc_create", "Gdi32.dll", "CreateCompatibleDC"),
    ("Gui", "get_dc", "User32.dll", "GetDC"),
    ("Gui", "window_create", "User32.dll", "CreateWindowExA"),
    ("Gui", "blit", "Gdi32.dll", "StretchDIBits"),
    ("Gui", "msg_peek", "User32.dll", "PeekMessageW"),
    ("Gui", "msg_translate", "User32.dll", "TranslateMessage"),
    ("Gui", "msg_dispatch", "User32.dll", "DispatchMessageW"),
    ("Gui", "is_window", "User32.dll", "IsWindow"),
    ("Gui", "window_destroy", "User32.dll", "DestroyWindow"),
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
    let policy: std::sync::Arc<str> = "omega::host::targets::windows".into();
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: std::sync::Arc::clone(&policy),
        checked: true,
    });

    plan.bindings.insert_many(
        WINDOWS_IMPORT_ROWS
            .iter()
            .map(|(capability, operation, library, symbol)| {
                windows_import(capability, operation, library, symbol, &policy)
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
    // The windowed-renderer surface (user32/gdi32). The encoders exist only for
    // x86_64 today; withholding the lowerings on other architectures turns a Gui
    // call into a clean UnsupportedHostCall diagnostic instead of a selection
    // panic (mirrors the carrier text ops, which are x86_64-only).
    if plan.target.architecture == omega_target::Architecture::X86_64 {
        insert_platform_lowering(
            plan,
            "*",
            "dc_create",
            [host_operation("Gui", "dc_create")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "get_dc",
            [host_operation("Gui", "get_dc")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "window_create",
            [host_operation("Gui", "window_create")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "blit",
            [host_operation("Gui", "blit")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "msg_peek",
            [host_operation("Gui", "msg_peek")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "msg_translate",
            [host_operation("Gui", "msg_translate")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "msg_dispatch",
            [host_operation("Gui", "msg_dispatch")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "is_window",
            [host_operation("Gui", "is_window")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "*",
            "window_destroy",
            [host_operation("Gui", "window_destroy")],
            PlatformCallData::None,
        );
    }
}

fn windows_import(
    capability: &str,
    operation: &str,
    library: &str,
    symbol: &str,
    policy: &std::sync::Arc<str>,
) -> HostBinding {
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Import {
            library: library.into(),
            symbol: symbol.into(),
        },
        // Share ONE policy allocation across every binding (all name the same
        // target path) -- an Arc refcount bump, not a fresh string per row.
        boundary_policy: std::sync::Arc::clone(policy),
    }
}
