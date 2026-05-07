mod darwin;
mod linux;
mod windows;

use crate::target::{NativeTarget, ObjectFormat};
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
    MutableOutputBuffer {
        byte_capacity: usize,
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

pub(super) fn insert_platform_lowering<const COUNT: usize>(
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

pub(super) fn host_operation(capability: &str, operation: &str) -> HostOperationReference {
    HostOperationReference {
        capability: capability.to_owned(),
        operation: operation.to_owned(),
    }
}
