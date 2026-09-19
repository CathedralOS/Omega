//! The current target operations program.
//!
//! This root describes program data at this resolution level. Its subordinate
//! areas own related facts; it does not contain transformation-stage objects.

pub use abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractReboundDynamicDispatch, AbstractResult, AbstractStoredDynamicDescriptor,
    AbstractStoredDynamicDispatch, CompletionClaimSource,
};
pub use calling_conventions::MachineRegister;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<TargetFunction>,
    /// The exact native-only callback arguments this plan's normalized
    /// foreign calls consume. Each row is admission-retained custody joined to
    /// its registrar by Terminal operation and registrar entry plan; a
    /// callback has no semantic `ValueId`, so it lives on the plan itself
    /// rather than inside an argument roster.
    pub native_callback_arguments: Vec<TargetNativeCallbackArgument>,
}

pub mod calls;
pub use calls::*;
pub mod control_flow;
pub use control_flow::*;
pub mod provenance;
pub use provenance::*;
pub mod boundary;
pub use boundary::*;
pub mod operations;
pub use operations::*;
pub mod values;
pub use values::*;
pub mod storage;
pub use storage::*;

#[cfg(test)]
mod tests;
