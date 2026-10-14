#![forbid(unsafe_code)]

//! Optimizer module role: crate map. target operations representation.
//!
//! Start at [`target_operations`], which defines the program root and maps the
//! subordinate control, value, call, storage, and evidence owners.

pub mod target_operations;
pub use abstract_operations::ValueBinding;
pub(crate) use target_operations::TargetX86ScalarFmaSettlement;
pub use target_operations::{
    AbstractResult, BoundaryByteSequenceArgument, BoundaryExecutionBinding, BoundaryRealization,
    BoundaryScalarArgument, BoundarySettlementBinding, BoundarySettlementRealization,
    CallSiteOwner, ClaimCompletionOnlyRealization, CompilerBuiltinExecution, CompletionClaimSource,
    DirectPortReadU8Realization, HostedExitProcessI32Realization, HostedReadByteRealization,
    HostedWriteByteI32Realization, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
    IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    LinuxWriteLineRealization, MachineRegister, MetadataOnlyPortRealization,
    MixedStructuralScalarFunctionAbi, NativeCallOrigin, NativeCallTarget,
    NearestIeeeFloatArithmeticRealization, NormalizedForeignCallBinding,
    NormalizedForeignCallMechanism, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, NormalizedSystemCallBinding,
    PlacedScalarAccessRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
    ScalarAbiValue, ScalarFunctionAbi, ScalarParameterLocation, TargetBooleanExpression,
    TargetBoundaryResult, TargetByteView, TargetCallResult, TargetControlBlock,
    TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlGraph,
    TargetControlSuccessor, TargetControlTerminator, TargetElementView, TargetElementViewRegion,
    TargetFloatResultPolicy, TargetFunction, TargetIeeeFloatFmaOperand, TargetIntegerExpression,
    TargetNativeCallbackArgument, TargetOperationPlan, TargetOperationPlanWithPlacedViewInputs,
    TargetPlacedCallAssociation, TargetPlacedCallBinding, TargetPlacedInterpretation,
    TargetPlacedViewInput, TargetReferenceOrigin, TargetReferenceResult,
    TargetScalarBlockParameter, TargetScalarBlockValue, TargetScalarExpression,
    TargetScalarImmediate, TargetStructuralArgument, TargetStructuralArgumentSource,
    TargetStructuralCaseSource, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
    TargetStructuralHomeRequirement, TargetStructuralParameter, TargetStructuralResultContract,
    TargetStructuralReturnSource, TargetStructuralRuntimeIndex, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource, TargetViewExtent, TerminalPsiProvenance, boundary,
    calls, control_flow, conventional_float_class_layout, operations, provenance,
    selector_carrier_type, storage, values,
};
