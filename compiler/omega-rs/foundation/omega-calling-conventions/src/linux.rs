use crate::{
    CallSignature, CallingPolicy, HostAbiPlan, HostBinding, HostBindingMechanism,
    HostBoundaryPolicy, PlatformCallData, ValueShape, evaluate_ordinary_boundary_entry_plan,
    host_operation, insert_platform_lowering,
};
use omega_target::Architecture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinuxSyscallNumbers {
    read: u32,
    write: u32,
    close: u32,
    pread64: u32,
    pwrite64: u32,
    lseek: u32,
    dup: u32,
    flock: u32,
    fsync: u32,
    ftruncate: u32,
    fchmod: u32,
    mkdirat: u32,
    unlinkat: u32,
    symlinkat: u32,
    linkat: u32,
    renameat: u32,
    getdents64: u32,
    fchmodat: u32,
    readlinkat: u32,
    fchown: u32,
    fstat: u32,
    newfstatat: u32,
    openat: u32,
    exit_group: u32,
    clock_gettime: u32,
    nanosleep: u32,
}

pub(crate) fn populate(plan: &mut HostAbiPlan) {
    let policy: std::sync::Arc<str> = "omega::host::targets::linux".into();
    plan.boundary_policies.insert(HostBoundaryPolicy {
        path: std::sync::Arc::clone(&policy),
        checked: true,
    });

    let syscall_numbers = linux_syscall_numbers(plan.target.architecture);
    plan.bindings.insert_many([
        linux_value_syscall(
            "Stdin",
            "read",
            "read",
            syscall_numbers.read,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Stdout",
            "write",
            "write",
            syscall_numbers.write,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Stderr",
            "write",
            "write",
            syscall_numbers.write,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_void_syscall(
            "Process",
            "exit_group",
            syscall_numbers.exit_group,
            1,
            &policy,
            plan.target.architecture,
        ),
        linux_clock_gettime_syscall(
            "monotonic_ticks",
            syscall_numbers.clock_gettime,
            &policy,
            plan.target.architecture,
        ),
        linux_clock_gettime_syscall(
            "wall_clock_raw",
            syscall_numbers.clock_gettime,
            &policy,
            plan.target.architecture,
        ),
        linux_timespec_syscall(
            "sleep",
            "nanosleep",
            syscall_numbers.nanosleep,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "open",
            "openat",
            syscall_numbers.openat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "open_create",
            "openat",
            syscall_numbers.openat,
            4,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "openat",
            "openat",
            syscall_numbers.openat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "read",
            "read",
            syscall_numbers.read,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "write",
            "write",
            syscall_numbers.write,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "pread",
            "pread64",
            syscall_numbers.pread64,
            4,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "pwrite",
            "pwrite64",
            syscall_numbers.pwrite64,
            4,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "close",
            "close",
            syscall_numbers.close,
            1,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "lseek",
            "lseek",
            syscall_numbers.lseek,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "fchmod",
            "fchmod",
            syscall_numbers.fchmod,
            2,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "mkdir",
            "mkdirat",
            syscall_numbers.mkdirat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "chmod",
            "fchmodat",
            syscall_numbers.fchmodat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "unlinkat",
            "unlinkat",
            syscall_numbers.unlinkat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "unlink",
            "unlinkat",
            syscall_numbers.unlinkat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "rmdir",
            "unlinkat",
            syscall_numbers.unlinkat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "readlink",
            "readlinkat",
            syscall_numbers.readlinkat,
            4,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "rename",
            "renameat",
            syscall_numbers.renameat,
            4,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "link",
            "linkat",
            syscall_numbers.linkat,
            5,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "symlink",
            "symlinkat",
            syscall_numbers.symlinkat,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "getdirentries64",
            "getdents64",
            syscall_numbers.getdents64,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "ftruncate",
            "ftruncate",
            syscall_numbers.ftruncate,
            2,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "fsync",
            "fsync",
            syscall_numbers.fsync,
            1,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "dup",
            "dup",
            syscall_numbers.dup,
            1,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "flock",
            "flock",
            syscall_numbers.flock,
            2,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "fchown",
            "fchown",
            syscall_numbers.fchown,
            3,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "fstat",
            "fstat",
            syscall_numbers.fstat,
            2,
            &policy,
            plan.target.architecture,
        ),
        linux_value_syscall(
            "Filesystem",
            "stat",
            "newfstatat",
            syscall_numbers.newfstatat,
            4,
            &policy,
            plan.target.architecture,
        ),
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
        "read_byte",
        [host_operation("Stdin", "read")],
        PlatformCallData::SingleByteRead,
    );
    insert_platform_lowering(
        plan,
        "*",
        "write_byte",
        [host_operation("Stdout", "write")],
        PlatformCallData::SingleByteWrite,
    );
    insert_platform_lowering(
        plan,
        "*",
        "exit_process",
        [host_operation("Process", "exit_group")],
        PlatformCallData::None,
    );
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks",
        [host_operation("Clock", "monotonic_ticks")],
        PlatformCallData::TimespecResult { clock_id: 1 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "tick_count",
        [host_operation("Clock", "monotonic_ticks")],
        PlatformCallData::TimespecResult { clock_id: 1 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "monotonic_ticks_per_second",
        [host_operation("Clock", "monotonic_ticks_per_second")],
        PlatformCallData::ConstantResult {
            value: 1_000_000_000,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_raw",
        [host_operation("Clock", "wall_clock_raw")],
        PlatformCallData::TimespecResult { clock_id: 0 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_units_per_second",
        [host_operation("Clock", "wall_clock_units_per_second")],
        PlatformCallData::ConstantResult {
            value: 1_000_000_000,
        },
    );
    insert_platform_lowering(
        plan,
        "*",
        "wall_clock_epoch_offset_seconds",
        [host_operation("Clock", "wall_clock_epoch_offset_seconds")],
        PlatformCallData::ConstantResult { value: 0 },
    );
    insert_platform_lowering(
        plan,
        "*",
        "sleep",
        [host_operation("Clock", "sleep")],
        PlatformCallData::TimespecArgument,
    );
    for (method, operation) in [
        ("read", "read"),
        ("write", "write"),
        ("read_at", "pread"),
        ("write_at", "pwrite"),
        ("close", "close"),
        ("seek", "lseek"),
        ("open_at", "openat"),
        ("unlink_at", "unlinkat"),
        ("set_file_permissions", "fchmod"),
        ("set_len", "ftruncate"),
        ("sync", "fsync"),
        ("sync_data", "fsync"),
        ("duplicate", "dup"),
        ("lock_file", "flock"),
        ("change_file_owner", "fchown"),
        ("read_file_metadata", "fstat"),
    ] {
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            method,
            [host_operation("Filesystem", operation)],
            PlatformCallData::None,
        );
    }
    for (method, trailing_flags) in [("read_metadata", 0), ("read_symlink_metadata", 256)] {
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            method,
            [host_operation("Filesystem", "stat")],
            PlatformCallData::ConstantArguments {
                leading: -100,
                trailing: trailing_flags,
            },
        );
    }
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_dir",
        [host_operation("Filesystem", "getdirentries64")],
        PlatformCallData::OmitTrailingArgument,
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "open",
        [host_operation("Filesystem", "open")],
        PlatformCallData::ConstantArgument { value: -100 },
    );
    for (method, operation, first_dirfd, trailing_flags) in [
        ("rename", "rename", Some(-100), None),
        ("hard_link", "link", Some(-100), Some(0)),
        ("symlink", "symlink", None, None),
    ] {
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            method,
            [host_operation("Filesystem", operation)],
            PlatformCallData::DirectoryRelativePathPair {
                first_dirfd,
                second_dirfd: -100,
                trailing_flags,
            },
        );
    }
    for method in ["remove", "remove_name"] {
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            method,
            [host_operation("Filesystem", "unlink")],
            PlatformCallData::ConstantArguments {
                leading: -100,
                trailing: 0,
            },
        );
    }
    for method in ["remove_dir", "remove_dir_name"] {
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            method,
            [host_operation("Filesystem", "rmdir")],
            PlatformCallData::ConstantArguments {
                leading: -100,
                trailing: 512,
            },
        );
    }
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "read_link",
        [host_operation("Filesystem", "readlink")],
        PlatformCallData::ConstantArgument { value: -100 },
    );
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "open_create",
        [host_operation("Filesystem", "open_create")],
        PlatformCallData::ConstantArgument { value: -100 },
    );
    for method in ["create_dir", "create_dir_name"] {
        insert_platform_lowering(
            plan,
            "FilesystemHost",
            method,
            [host_operation("Filesystem", "mkdir")],
            PlatformCallData::ConstantArgument { value: -100 },
        );
    }
    insert_platform_lowering(
        plan,
        "FilesystemHost",
        "set_permissions",
        [host_operation("Filesystem", "chmod")],
        PlatformCallData::ConstantArgument { value: -100 },
    );
}

fn linux_syscall_numbers(architecture: Architecture) -> LinuxSyscallNumbers {
    match architecture {
        Architecture::Aarch64 => LinuxSyscallNumbers {
            read: 63,
            write: 64,
            close: 57,
            pread64: 67,
            pwrite64: 68,
            lseek: 62,
            dup: 23,
            flock: 32,
            fsync: 82,
            ftruncate: 46,
            fchmod: 52,
            mkdirat: 34,
            unlinkat: 35,
            symlinkat: 36,
            linkat: 37,
            renameat: 38,
            getdents64: 61,
            fchmodat: 53,
            readlinkat: 78,
            fchown: 55,
            fstat: 80,
            newfstatat: 79,
            openat: 56,
            exit_group: 94,
            clock_gettime: 113,
            nanosleep: 101,
        },
        Architecture::X86_64 => LinuxSyscallNumbers {
            read: 0,
            write: 1,
            close: 3,
            pread64: 17,
            pwrite64: 18,
            lseek: 8,
            dup: 32,
            flock: 73,
            fsync: 74,
            ftruncate: 77,
            fchmod: 91,
            mkdirat: 258,
            unlinkat: 263,
            symlinkat: 266,
            linkat: 265,
            renameat: 264,
            getdents64: 217,
            fchmodat: 268,
            readlinkat: 267,
            fchown: 93,
            fstat: 5,
            newfstatat: 262,
            openat: 257,
            exit_group: 231,
            clock_gettime: 228,
            nanosleep: 35,
        },
    }
}

pub fn linux_clock_gettime_syscall_number(architecture: Architecture) -> u32 {
    linux_syscall_numbers(architecture).clock_gettime
}

pub fn linux_nanosleep_syscall_number(architecture: Architecture) -> u32 {
    linux_syscall_numbers(architecture).nanosleep
}

fn linux_clock_gettime_syscall(
    operation: &str,
    number: u32,
    policy: &std::sync::Arc<str>,
    architecture: Architecture,
) -> HostBinding {
    let calling_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        // clockid_t is widened into the syscall word; the second word is the
        // compiler-owned address of the temporary `timespec`.
        parameters: vec![word, word],
        result: Some(word),
    };
    let boundary_entry_plan = evaluate_ordinary_boundary_entry_plan(calling_policy, &signature)
        .expect("the built-in Linux clock_gettime signature must have a syscall plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names("Clock", operation),
        mechanism: HostBindingMechanism::Syscall {
            name: "clock_gettime".into(),
            number,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan,
    }
}

fn linux_timespec_syscall(
    operation: &str,
    name: &str,
    number: u32,
    policy: &std::sync::Arc<str>,
    architecture: Architecture,
) -> HostBinding {
    let calling_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![word, word],
        result: Some(word),
    };
    let boundary_entry_plan = evaluate_ordinary_boundary_entry_plan(calling_policy, &signature)
        .expect("the built-in Linux timespec syscall signature must have a syscall plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names("Clock", operation),
        mechanism: HostBindingMechanism::Syscall {
            name: name.into(),
            number,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan,
    }
}

fn linux_value_syscall(
    capability: &str,
    operation: &str,
    name: &str,
    number: u32,
    parameter_count: usize,
    policy: &std::sync::Arc<str>,
    architecture: Architecture,
) -> HostBinding {
    let calling_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let word = ValueShape::integer(8, 8);
    let signature = CallSignature {
        parameters: vec![word; parameter_count],
        result: Some(word),
    };
    let boundary_entry_plan = evaluate_ordinary_boundary_entry_plan(calling_policy, &signature)
        .expect("the built-in Linux value syscall signature must have a syscall plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Syscall {
            name: name.into(),
            number,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan,
    }
}

fn linux_void_syscall(
    capability: &str,
    operation: &str,
    number: u32,
    parameter_count: usize,
    policy: &std::sync::Arc<str>,
    architecture: Architecture,
) -> HostBinding {
    let calling_policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); parameter_count],
        result: None,
    };
    let boundary_entry_plan = evaluate_ordinary_boundary_entry_plan(calling_policy, &signature)
        .expect("the built-in Linux void syscall signature must have a syscall plan")
        .plan()
        .clone();
    HostBinding {
        operation_key: crate::HostOperationKey::from_names(capability, operation),
        mechanism: HostBindingMechanism::Syscall {
            name: operation.into(),
            number,
        },
        boundary_policy: std::sync::Arc::clone(policy),
        boundary_entry_plan,
    }
}
