use crate::{
    HostAbiPlan, HostBinding, HostBindingMechanism, HostBoundaryPolicy, PlatformCallData,
    host_operation, insert_platform_lowering,
};

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    let policy: std::sync::Arc<str> = "omega::host::targets::darwin".into();
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: std::sync::Arc::clone(&policy),
        checked: true,
    });

    plan.bindings.insert_many([
        darwin_import("Stdin", "read", "_read", &policy),
        darwin_import("Stdout", "write", "_write", &policy),
        darwin_import("Stderr", "write", "_write", &policy),
        darwin_import("Process", "exit", "_exit", &policy),
        darwin_import("Filesystem", "open", "_open", &policy),
        darwin_import("Filesystem", "creat", "_creat", &policy),
        darwin_import("Filesystem", "read", "_read", &policy),
        darwin_import("Filesystem", "write", "_write", &policy),
        darwin_import("Filesystem", "close", "_close", &policy),
        darwin_import("Filesystem", "unlink", "_unlink", &policy),
        darwin_import("Filesystem", "lseek", "_lseek", &policy),
        darwin_import("Filesystem", "mkdir", "_mkdir", &policy),
        darwin_import("Filesystem", "rmdir", "_rmdir", &policy),
        darwin_import("Filesystem", "rename", "_rename", &policy),
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
        "exit_process",
        [host_operation("Process", "exit")],
        PlatformCallData::None,
    );
    // std::fs — the RAW, VALUE-RETURNING boundary layer (each op returns its
    // syscall result: fd / byte count / rc; a thin Omega layer wraps these into
    // File/result enums). HUMAN method names (create/open/read/write/close/
    // remove) — NO legacy C abbreviations in the Omega surface; the ugly libc
    // spellings (`_creat`,`_unlink`) live only in the binding symbols above.
    // Registered under the raw trait `FilesystemHost` (not `*`) so `write`/`read`
    // win the exact-platform lookup over Console's wildcard `write`. All marshal
    // declared args straight through (PlatformCallData::None); the value-returning
    // result store is driven by `HostOperationKey::returns_value()`.
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "open",
        [host_operation("Filesystem", "open")],
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
}

fn darwin_import(
    capability: &str,
    operation: &str,
    symbol: &str,
    policy: &std::sync::Arc<str>,
) -> HostBinding {
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Import {
            library: "libSystem.B.dylib".into(),
            symbol: symbol.into(),
        },
        boundary_policy: std::sync::Arc::clone(policy),
    }
}
