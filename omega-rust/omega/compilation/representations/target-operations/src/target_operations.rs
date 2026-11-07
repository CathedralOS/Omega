//! The current target operations program.
//!
//! This root describes program data at this resolution level. Its subordinate
//! areas own related facts; it does not contain transformation-stage objects.

pub use abstract_operations::{AbstractResult, CompletionClaimSource};
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
    /// Native storage geometry of the borrowed boundary inputs and owned
    /// roots this plan's foreign calls reach, captured from the structural
    /// catalog before lowering consumed it.
    pub borrowed_storage_layouts: Vec<runtime_abi::NativeBorrowedStorageLayout>,
}

impl TargetOperationPlan {
    /// The target function for `machine`, when the plan carries it.
    pub fn function(&self, machine: MachineId) -> Option<&TargetFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }
}

pub mod calls;
pub use calls::{
    MixedStructuralScalarFunctionAbi, NativeCallTarget, ScalarAbiValue, ScalarFunctionAbi,
    TargetAccessCallAssociation, TargetAccessCallBinding, TargetAccessInterpretation,
    TargetAccessViewInput, TargetBoundaryResult, TargetCallResult, TargetNativeCallbackArgument,
    TargetOperationPlanWithAccessViewInputs, TargetStructuralResultContract,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, selector_carrier_type,
};
pub mod control_flow;
pub use control_flow::{
    TargetControlBlock, TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlGraph,
    TargetControlSuccessor, TargetControlTerminator, TargetFunction, TargetScalarBlockParameter,
    TargetStructuralCaseSource, TargetStructuralReturnSource,
};
pub mod provenance;
mod reference_root;
pub use provenance::{CallSiteOwner, InstallationCallRole, TerminalPsiProvenance};
pub use reference_root::ReferenceRoot;
pub mod boundary;
mod boundary_origins;
mod descriptor_input;
pub use boundary::{
    AccessorScalarAccessRealization, BoundaryByteSequenceArgument, BoundaryExecutionBinding,
    BoundaryRealization, BoundaryScalarArgument, BoundarySettlementBinding,
    BoundarySettlementRealization, ClaimCompletionOnlyRealization, CompilerBuiltinExecution,
    DirectPortReadU8Realization, ExternalCompletionBinding, ForeignCallOperands,
    HostedExitProcessI32Realization, HostedReadByteRealization, HostedWriteByteI32Realization,
    IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation, IeeeFloatIntegerCarrier,
    IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain, LinuxWriteLineRealization,
    MetadataOnlyPortRealization, NearestIeeeFloatArithmeticRealization,
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, NormalizedSystemCallBinding, ProviderExecutionBinding,
    ProviderPlanReportIdentity, conventional_atomic_once_layout, conventional_float_class_layout,
};
pub use boundary_origins::{BoundaryOriginFacts, ProgramEntryOrigin};
pub use descriptor_input::{
    DescriptorInputGuard, descriptor_input_guard, descriptor_input_guard_for_call,
};
pub mod operations;
pub use operations::{
    NativeCallOrigin, TargetFloatResultPolicy, TargetInstallationCall,
    TargetStructuralRuntimeIndex, TargetUnitOperation,
};
pub mod values;
pub use values::{
    TargetBooleanExpression, TargetByteView, TargetElementView, TargetElementViewRegion,
    TargetIntegerExpression, TargetReferenceOrigin, TargetReferenceOriginBase,
    TargetReferenceResult, TargetScalarBlockValue, TargetScalarExpression, TargetScalarImmediate,
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralParameter,
    TargetViewExtent,
};
pub mod storage;
pub use storage::{
    ScalarParameterLocation, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
    TargetStructuralHomeRequirement, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource,
};

#[cfg(test)]
mod tests;
