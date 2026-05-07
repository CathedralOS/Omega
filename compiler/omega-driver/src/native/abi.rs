use crate::native::target::{NativeTarget, ObjectFormat};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAbiPlan {
    pub target: NativeTarget,
    pub bindings: Arena<HostBinding>,
    pub host_operations: Arena<HostOperationReference>,
    pub platform_call_lowerings: Arena<PlatformCallLowering>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    pub capability: String,
    pub operation: String,
    pub mechanism: HostBindingMechanism,
    pub trust_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCallLowering {
    pub platform: String,
    pub state: String,
    pub operations: HandleSpan<HostOperationReference>,
    pub data: PlatformCallData,
}

impl Default for PlatformCallLowering {
    fn default() -> Self {
        Self {
            platform: String::new(),
            state: String::new(),
            operations: HandleSpan::empty(),
            data: PlatformCallData::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlatformCallData {
    #[default]
    None,
    FirstTextArgument {
        append_newline: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostOperationReference {
    pub capability: String,
    pub operation: String,
}

impl Default for HostOperationReference {
    fn default() -> Self {
        Self {
            capability: String::new(),
            operation: String::new(),
        }
    }
}

impl Default for HostBinding {
    fn default() -> Self {
        Self {
            capability: String::new(),
            operation: String::new(),
            mechanism: HostBindingMechanism::Import {
                library: String::new(),
                symbol: String::new(),
            },
            trust_policy: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostBindingMechanism {
    Import { library: String, symbol: String },
    Syscall { name: String, number: u32 },
}

pub fn build_host_abi_plan(target: NativeTarget) -> HostAbiPlan {
    let mut plan = HostAbiPlan {
        target,
        bindings: Arena::new(),
        host_operations: Arena::new(),
        platform_call_lowerings: Arena::new(),
    };

    match target.object_format {
        ObjectFormat::Coff => {
            plan.bindings.insert_many([
                windows_import("Stdout", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
                windows_import("Stdout", "write_file", "Kernel32.dll", "WriteFile"),
                windows_import("Process", "exit_process", "Kernel32.dll", "ExitProcess"),
            ]);
            insert_platform_lowering(
                &mut plan,
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
                &mut plan,
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
                &mut plan,
                "*",
                "exit_process",
                [host_operation("Process", "exit_process")],
                PlatformCallData::None,
            );
        }
        ObjectFormat::Elf => {
            plan.bindings.insert_many([
                linux_syscall("Stdout", "write", 1),
                linux_syscall("Process", "exit_group", 231),
            ]);
            insert_platform_lowering(
                &mut plan,
                "*",
                "write_line",
                [host_operation("Stdout", "write")],
                PlatformCallData::FirstTextArgument {
                    append_newline: true,
                },
            );
            insert_platform_lowering(
                &mut plan,
                "*",
                "write",
                [host_operation("Stdout", "write")],
                PlatformCallData::FirstTextArgument {
                    append_newline: false,
                },
            );
            insert_platform_lowering(
                &mut plan,
                "*",
                "exit_process",
                [host_operation("Process", "exit_group")],
                PlatformCallData::None,
            );
        }
        ObjectFormat::MachO => {
            plan.bindings.insert_many([
                darwin_import("Stdout", "write", "libSystem.dylib", "_write"),
                darwin_import("Process", "exit", "libSystem.dylib", "_exit"),
            ]);
            insert_platform_lowering(
                &mut plan,
                "*",
                "write_line",
                [host_operation("Stdout", "write")],
                PlatformCallData::FirstTextArgument {
                    append_newline: true,
                },
            );
            insert_platform_lowering(
                &mut plan,
                "*",
                "write",
                [host_operation("Stdout", "write")],
                PlatformCallData::FirstTextArgument {
                    append_newline: false,
                },
            );
            insert_platform_lowering(
                &mut plan,
                "*",
                "exit_process",
                [host_operation("Process", "exit")],
                PlatformCallData::None,
            );
        }
    }

    plan
}

fn insert_platform_lowering<const COUNT: usize>(
    plan: &mut HostAbiPlan,
    platform: &str,
    state: &str,
    operations: [HostOperationReference; COUNT],
    data: PlatformCallData,
) {
    let operations = plan.host_operations.insert_many(operations);
    plan.platform_call_lowerings.insert(PlatformCallLowering {
        platform: platform.to_owned(),
        state: state.to_owned(),
        operations,
        data,
    });
}

fn host_operation(capability: &str, operation: &str) -> HostOperationReference {
    HostOperationReference {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
    }
}

fn windows_import(capability: &str, operation: &str, library: &str, symbol: &str) -> HostBinding {
    HostBinding {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
        mechanism: HostBindingMechanism::Import {
            library: library.to_owned(),
            symbol: symbol.to_owned(),
        },
        trust_policy: "omega::host::windows".to_owned(),
    }
}

fn linux_syscall(capability: &str, operation: &str, number: u32) -> HostBinding {
    HostBinding {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
        mechanism: HostBindingMechanism::Syscall {
            name: operation.to_owned(),
            number,
        },
        trust_policy: "omega::host::linux".to_owned(),
    }
}

fn darwin_import(capability: &str, operation: &str, library: &str, symbol: &str) -> HostBinding {
    HostBinding {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
        mechanism: HostBindingMechanism::Import {
            library: library.to_owned(),
            symbol: symbol.to_owned(),
        },
        trust_policy: "omega::host::darwin".to_owned(),
    }
}
