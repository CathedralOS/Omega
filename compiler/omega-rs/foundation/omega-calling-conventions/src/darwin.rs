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
        darwin_import("Filesystem", "read", "_read", &policy),
        darwin_import("Filesystem", "write", "_write", &policy),
        darwin_import("Filesystem", "close", "_close", &policy),
        darwin_import("Filesystem", "unlink", "_unlink", &policy),
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
    // File/outcome enums). Method names are distinct from Console's (`write` is
    // already a Console lowering under the `*` wildcard), so `read`/`write` are
    // spelled `read_bytes`/`write_bytes` here. All marshal declared args
    // straight through (PlatformCallData::None); the value-returning result
    // store is driven by `HostOperationKey::returns_value()`.
    insert_platform_lowering(
        plan,
        "*",
        "open",
        [host_operation("Filesystem", "open")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "read_bytes",
        [host_operation("Filesystem", "read")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_bytes",
        [host_operation("Filesystem", "write")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "close",
        [host_operation("Filesystem", "close")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "unlink",
        [host_operation("Filesystem", "unlink")],
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
