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
    TargetBoundaryResult, TargetCallResult, TargetNativeCallbackArgument,
    TargetOperationPlanWithPlacedViewInputs, TargetPlacedCallAssociation, TargetPlacedCallBinding,
    TargetPlacedInterpretation, TargetPlacedViewInput, TargetStructuralResultContract,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, selector_carrier_type,
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
pub(crate) use boundary::TargetX86ScalarFmaSettlement;
pub use boundary::{
    BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, BoundarySettlementBinding, BoundarySettlementRealization,
    ClaimCompletionOnlyRealization, CompilerBuiltinExecution, DirectPortReadU8Realization,
    ForeignCallOperands, HostedExitProcessI32Realization, HostedReadByteRealization,
    HostedWriteByteI32Realization, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
    IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    LinuxWriteLineRealization, MetadataOnlyPortRealization, NearestIeeeFloatArithmeticRealization,
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, NormalizedSystemCallBinding,
    PlacedScalarAccessRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
    TargetIeeeFloatFmaOperand, conventional_float_class_layout,
};
pub mod operations;
pub use operations::{
    NativeCallOrigin, TargetFloatResultPolicy, TargetStructuralRuntimeIndex, TargetUnitOperation,
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
