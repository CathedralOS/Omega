mod darwin;
mod linux;
mod windows;

use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_target::{NativeTarget, ObjectFormat};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HostOperationKey {
    pub capability: HostCapability,
    pub operation: HostOperation,
}

impl HostOperationKey {
    pub const fn new(capability: HostCapability, operation: HostOperation) -> Self {
        Self {
            capability,
            operation,
        }
    }

    pub fn capability_name(self) -> &'static str {
        self.capability.name()
    }

    pub fn operation_name(self) -> &'static str {
        self.operation.name()
    }

    pub fn from_names(capability: &str, operation: &str) -> Self {
        Self::new(
            HostCapability::from_name(capability),
            HostOperation::from_name(operation),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostCapability {
    #[default]
    Unknown,
    Process,
    Stdin,
    Stdout,
}

impl HostCapability {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Process" => Self::Process,
            "Stdin" => Self::Stdin,
            "Stdout" => Self::Stdout,
            _ => Self::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
            Self::Process => "Process",
            Self::Stdin => "Stdin",
            Self::Stdout => "Stdout",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostOperation {
    #[default]
    Unknown,
    Exit,
    ExitGroup,
    ExitProcess,
    GetStdHandle,
    Read,
    ReadFile,
    Write,
    WriteFile,
}

impl HostOperation {
    pub fn from_name(name: &str) -> Self {
        match name {
            "exit" => Self::Exit,
            "exit_group" => Self::ExitGroup,
            "exit_process" => Self::ExitProcess,
            "get_std_handle" => Self::GetStdHandle,
            "read" => Self::Read,
            "read_file" => Self::ReadFile,
            "write" => Self::Write,
            "write_file" => Self::WriteFile,
            _ => Self::Unknown,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Unknown => "<unknown>",
            Self::Exit => "exit",
            Self::ExitGroup => "exit_group",
            Self::ExitProcess => "exit_process",
            Self::GetStdHandle => "get_std_handle",
            Self::Read => "read",
            Self::ReadFile => "read_file",
            Self::Write => "write",
            Self::WriteFile => "write_file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAbiPlan {
    pub target: NativeTarget,
    pub bindings: Arena<HostBinding>,
    pub host_operations: Arena<HostOperationReference>,
    pub platform_call_lowerings: Arena<PlatformCallLowering>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostBinding {
    pub operation_key: HostOperationKey,
    pub mechanism: HostBindingMechanism,
    pub trust_policy: Arc<str>,
}

impl Default for HostBinding {
    fn default() -> Self {
        Self {
            operation_key: HostOperationKey::default(),
            mechanism: HostBindingMechanism::Import {
                library: Arc::from(""),
                symbol: Arc::from(""),
            },
            trust_policy: Arc::from(""),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostBindingMechanism {
    Import {
        library: Arc<str>,
        symbol: Arc<str>,
    },
    Syscall {
        name: Arc<str>,
        number: u32,
        number_register: u8,
        supervisor_call: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCallLowering {
    pub platform: Arc<str>,
    pub state: Arc<str>,
    pub operations: HandleSpan<HostOperationReference>,
    pub data: PlatformCallData,
}

pub type PlatformCallLoweringHandle = Handle<PlatformCallLowering>;

impl Default for PlatformCallLowering {
    fn default() -> Self {
        Self {
            platform: Arc::from(""),
            state: Arc::from(""),
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
    MutableOutputBuffer {
        byte_capacity: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostOperationReference {
    pub key: HostOperationKey,
}

impl Default for HostOperationReference {
    fn default() -> Self {
        Self {
            key: HostOperationKey::default(),
        }
    }
}

pub fn build_host_abi_plan(target: NativeTarget) -> HostAbiPlan {
    let mut plan = HostAbiPlan {
        target,
        bindings: Arena::new(),
        host_operations: Arena::new(),
        platform_call_lowerings: Arena::new(),
    };

    match target.object_format {
        ObjectFormat::Coff => windows::populate(&mut plan),
        ObjectFormat::Elf => linux::populate(&mut plan),
        ObjectFormat::MachO => darwin::populate(&mut plan),
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
        platform: Arc::from(platform),
        state: Arc::from(state),
        operations,
        data,
    });
}

fn host_operation(capability: &str, operation: &str) -> HostOperationReference {
    HostOperationReference {
        key: HostOperationKey::from_names(capability, operation),
    }
}

pub fn host_operation_fixed_leading_immediate(
    plan: &HostAbiPlan,
    operation_key: HostOperationKey,
) -> Option<i64> {
    match (
        plan.target.object_format,
        operation_key.capability,
        operation_key.operation,
    ) {
        (ObjectFormat::Coff, HostCapability::Stdout, HostOperation::GetStdHandle) => Some(-11),
        (ObjectFormat::Coff, HostCapability::Stdin, HostOperation::GetStdHandle) => Some(-10),
        _ => None,
    }
}
