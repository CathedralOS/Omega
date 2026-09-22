#![forbid(unsafe_code)]

//! Optimizer module role: crate map. target operations representation.
//!
//! Start at [`target_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub mod target_operations;
pub use abstract_operations::ValueBinding;
pub use target_operations::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource,
    AbstractReboundDynamicDispatch, AbstractResult, AbstractStoredDynamicDescriptor,
    AbstractStoredDynamicDispatch, BoundaryByteSequenceArgument, BoundaryExecutionBinding,
    BoundaryRealization, BoundaryScalarArgument, BoundarySettlementBinding,
    BoundarySettlementRealization, CallSiteOwner, ClaimCompletionOnlyRealization,
    CompilerBuiltinExecution, CompletionClaimSource, DirectPortReadU8Realization,
    HostedExitProcessI32Realization, HostedReadByteRealization, HostedWriteByteI32Realization,
    LinuxWriteLineRealization, MachineRegister, MetadataOnlyPortRealization,
    MixedStructuralScalarFunctionAbi, NativeCallOrigin, NormalizedForeignCallBinding,
    NormalizedForeignScalarArgument, NormalizedForeignStructuralArgument, ProviderExecutionBinding,
    ProviderPlanReportIdentity, ScalarAbiValue, ScalarFunctionAbi, ScalarParameterLocation,
    TargetBooleanExpression, TargetBoundaryResult, TargetByteView, TargetCallResult,
    TargetControlBlock, TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlGraph,
    TargetControlSuccessor, TargetControlTerminator, TargetDynamicDescriptorArgument,
    TargetDynamicDescriptorInstanceArgument, TargetDynamicDescriptorInstanceSource,
    TargetDynamicDescriptorParameterAbi, TargetFunction, TargetIeeeFloatFmaOperand,
    TargetIntegerExpression, TargetNativeCallbackArgument, TargetOperationPlan,
    TargetOperationPlanWithPlacedViewInputs, TargetPlacedViewInput, TargetReferenceResult,
    TargetScalarBlockParameter, TargetScalarBlockValue, TargetScalarExpression,
    TargetScalarImmediate, TargetStructuralArgument, TargetStructuralArgumentSource,
    TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
    TargetStructuralParameter, TargetStructuralReturnSource, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource, TargetX86ScalarFmaSettlement, TerminalPsiProvenance,
    boundary, calls, control_flow, operations, provenance, storage, values,
};
