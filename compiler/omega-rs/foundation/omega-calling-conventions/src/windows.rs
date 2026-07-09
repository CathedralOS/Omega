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
    // std::time TimeHost seam (rung 5): out-param u64 reads (the constants
    // wall_clock_units_per_second / wall_clock_epoch_offset_seconds have NO
    // import row -- they lower as ConstantResult, no call at all).
    ("Clock", "monotonic_ticks", "Kernel32.dll", "QueryPerformanceCounter"),
    ("Clock", "monotonic_ticks_per_second", "Kernel32.dll", "QueryPerformanceFrequency"),
    ("Clock", "wall_clock_raw", "Kernel32.dll", "GetSystemTimePreciseAsFileTime"),
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
    ("Gui", "foreground_window", "User32.dll", "GetForegroundWindow"),
    // std::fs raw seam (the windows_x64 mirror of darwin's libSystem rows):
    // msvcrt's POSIX-shaped CRT calls match the raw seam's value-returning
    // fd/count/rc surface directly (same arg shapes as the darwin libc calls),
    // so the general import-call encoder marshals them unchanged. The ops with
    // NO clean msvcrt equivalent (pread/pwrite, *at, link/symlink/readlink,
    // read_dir, flock, chown, futimens, realpath) and the stat family (whose
    // per-OS record layout the portable decode does not answer yet) keep the
    // clean "no native lowering" diagnostic.
    ("Filesystem", "open", "msvcrt.dll", "_open"),
    // `open_create` = `_open(path, flags, mode)` -- unfenced 2026-07-08 now that
    // the wrapper composes msvcrt flag words from the per-target `FilesystemHost`
    // provides values (create_new/open_with no longer emit darwin O_CREAT 0x200,
    // which is msvcrt O_TRUNC). msvcrt `_open` takes the create `mode` as a
    // trailing variadic int; on win64 it lands in a normal arg register, so the
    // general import-call encoder marshals all three args like any Win64 call.
    ("Filesystem", "open_create", "msvcrt.dll", "_open"),
    ("Filesystem", "creat", "msvcrt.dll", "_creat"),
    ("Filesystem", "read", "msvcrt.dll", "_read"),
    ("Filesystem", "write", "msvcrt.dll", "_write"),
    ("Filesystem", "close", "msvcrt.dll", "_close"),
    ("Filesystem", "unlink", "msvcrt.dll", "_unlink"),
    ("Filesystem", "lseek", "msvcrt.dll", "_lseeki64"),
    ("Filesystem", "mkdir", "msvcrt.dll", "_mkdir"),
    ("Filesystem", "rmdir", "msvcrt.dll", "_rmdir"),
    ("Filesystem", "rename", "msvcrt.dll", "rename"),
    ("Filesystem", "dup", "msvcrt.dll", "_dup"),
    ("Filesystem", "fsync", "msvcrt.dll", "_commit"),
    ("Filesystem", "chmod", "msvcrt.dll", "_chmod"),
    ("Filesystem", "read_errno", "msvcrt.dll", "_errno"),
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
    // The std::time seam (TASKS_TIME.md rung 5, D11): three OUT-PARAM u64
    // reads (QueryPerformanceCounter/-Frequency, GetSystemTimePreciseAsFileTime)
    // plus the two wall-clock calibration CONSTANTS -- FILETIME ticks at 10^7
    // per second, offset 11_644_473_600 s (1601 -> 1970). The lowering layer
    // never does arithmetic; wrapper code divides by these.
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks",
        [host_operation("Clock", "monotonic_ticks")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks_per_second",
        [host_operation("Clock", "monotonic_ticks_per_second")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_raw",
        [host_operation("Clock", "wall_clock_raw")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_units_per_second",
        [host_operation("Clock", "wall_clock_units_per_second")],
        PlatformCallData::ConstantResult { value: 10000000 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_epoch_offset_seconds",
        [host_operation("Clock", "wall_clock_epoch_offset_seconds")],
        PlatformCallData::ConstantResult {
            value: 11644473600,
        },
    );
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
        // `GetForegroundWindow()` -- lets a pump scope the GLOBAL
        // GetAsyncKeyState to its own window (an unfocused app must not
        // treat a desktop-wide ESC as its quit key).
        insert_platform_lowering(
            plan,
            "*",
            "foreground_window",
            [host_operation("Gui", "foreground_window")],
            PlatformCallData::None,
        );
        // std::fs raw seam -- registered under the raw trait `FilesystemHost`
        // (not `*`) so `write`/`read` win the exact-platform lookup over
        // Console's wildcard entries (same discipline as darwin.rs). All
        // marshal declared args straight through; the value-returning result
        // store is driven by `HostOperationKey::returns_value()`. x86_64-gated
        // with the Gui block: the encoders ride the general Win64 import call.
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "open",
            [host_operation("Filesystem", "open")],
            PlatformCallData::None,
        );
        // `open_create` unfenced 2026-07-08: the wrapper now composes msvcrt
        // flag words from the per-target `FilesystemHost` provides values, so
        // create_new/open_with no longer risk the darwin-O_CREAT-is-msvcrt-
        // O_TRUNC silent truncation. `_open(path, flags, mode)` rides the same
        // general import call as `open` (the trailing mode is a normal win64
        // arg register, not stack-passed like darwin arm64's variadic).
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "open_create",
            [host_operation("Filesystem", "open_create")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "create",
            [host_operation("Filesystem", "creat")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "read",
            [host_operation("Filesystem", "read")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "write",
            [host_operation("Filesystem", "write")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "close",
            [host_operation("Filesystem", "close")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "remove",
            [host_operation("Filesystem", "unlink")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "seek",
            [host_operation("Filesystem", "lseek")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "create_dir",
            [host_operation("Filesystem", "mkdir")],
            PlatformCallData::None,
        );
        // The TRUSTED plain-name variant (D-at trust class; create_dir_all's
        // NUL-terminated prefix scratch) -- same native row.
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "create_dir_name",
            [host_operation("Filesystem", "mkdir")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "remove_dir",
            [host_operation("Filesystem", "rmdir")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "rename",
            [host_operation("Filesystem", "rename")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "duplicate",
            [host_operation("Filesystem", "dup")],
            PlatformCallData::None,
        );
        // `sync`/`sync_data` both map to `_commit` (msvcrt's fsync analogue),
        // mirroring darwin's shared-fsync fallback.
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "sync",
            [host_operation("Filesystem", "fsync")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "sync_data",
            [host_operation("Filesystem", "fsync")],
            PlatformCallData::None,
        );
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "set_permissions",
            [host_operation("Filesystem", "chmod")],
            PlatformCallData::None,
        );
        // errno accessor: `_errno()` returns `&errno` (the same int*-returning
        // shape as darwin's `___error()`); the value-returning lowering derefs
        // the returned pointer once (see `dereferences_result`).
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            "errno",
            [host_operation("Filesystem", "read_errno")],
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
