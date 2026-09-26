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
pub use calls::{
    MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi, TargetBoundaryResult,
    TargetCallResult, TargetDynamicDescriptorArgument, TargetDynamicDescriptorInstanceArgument,
    TargetDynamicDescriptorInstanceSource, TargetDynamicDescriptorParameterAbi,
    TargetNativeCallbackArgument, TargetOperationPlanWithPlacedViewInputs, TargetPlacedViewInput,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument,
};
pub mod control_flow;
pub use control_flow::{
    TargetControlBlock, TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlGraph,
    TargetControlSuccessor, TargetControlTerminator, TargetFunction, TargetScalarBlockParameter,
    TargetStructuralCaseSource, TargetStructuralReturnSource,
};
pub mod provenance;
pub use provenance::{CallSiteOwner, TerminalPsiProvenance};
pub mod boundary;
pub use boundary::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, BoundarySettlementBinding, BoundarySettlementRealization,
    ClaimCompletionOnlyRealization, CompilerBuiltinExecution, DirectPortReadU8Realization,
    HostedExitProcessI32Realization, HostedReadByteRealization, HostedWriteByteI32Realization,
    LinuxWriteLineRealization, MetadataOnlyPortRealization, NormalizedForeignCallBinding,
    NormalizedForeignScalarArgument, NormalizedForeignStructuralArgument, ProviderExecutionBinding,
    ProviderPlanReportIdentity, TargetIeeeFloatFmaOperand, TargetX86ScalarFmaSettlement,
};
pub mod operations;
pub use operations::{NativeCallOrigin, TargetStructuralRuntimeIndex, TargetUnitOperation};
pub mod values;
pub use values::{
    TargetBooleanExpression, TargetByteView, TargetElementView, TargetIntegerExpression,
    TargetReferenceResult, TargetScalarBlockValue, TargetScalarExpression, TargetScalarImmediate,
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralParameter,
};
pub mod storage;
pub use storage::{
    ScalarParameterLocation, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
    TargetStructuralHomeRequirement, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource,
};

#[cfg(test)]
mod tests;
