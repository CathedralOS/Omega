use crate::native::target::{NativeTarget, ObjectFormat};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAbiPlan {
    pub target: NativeTarget,
    pub bindings: Arena<HostBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    pub capability: String,
    pub operation: String,
    pub mechanism: HostBindingMechanism,
    pub trust_policy: String,
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
    };

    match target.object_format {
        ObjectFormat::Coff => {
            plan.bindings.insert_many([
                windows_import("Stdout", "get_std_handle", "Kernel32.dll", "GetStdHandle"),
                windows_import("Stdout", "write_file", "Kernel32.dll", "WriteFile"),
                windows_import("Process", "exit_process", "Kernel32.dll", "ExitProcess"),
            ]);
        }
        ObjectFormat::Elf => {
            plan.bindings.insert_many([
                linux_syscall("Stdout", "write", 1),
                linux_syscall("Process", "exit_group", 231),
            ]);
        }
        ObjectFormat::MachO => {
            plan.bindings.insert_many([
                darwin_import("Stdout", "write", "libSystem.dylib", "_write"),
                darwin_import("Process", "exit", "libSystem.dylib", "_exit"),
            ]);
        }
    }

    plan
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
