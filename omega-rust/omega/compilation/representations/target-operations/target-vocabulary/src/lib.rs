#![forbid(unsafe_code)]

/*
The records a target program is made of, without the program.

Stage 02 lowers to target operations, then prepares the execution plan from
them; instruction selection and every later form carry many of the same
records (call contracts and argument sources, boundary realizations, call-site
owners, structural homes, value expressions). They are defined here, apart
from `TargetOperationPlan` and its control graph in `target-operations`, so a
form after the plan can name a record without being able to name stage 02's
working program.
*/

pub use calling_conventions::MachineRegister;
pub use installation_evidence::InstalledAssumedBindingEvidence;
pub use native_vocabulary::completion_claims::CompletionClaimSource;
pub use native_vocabulary::values::{AbstractResult, ValueBinding};

pub mod calls;
pub use calls::{
    MixedStructuralScalarFunctionAbi, NativeCallTarget, ScalarAbiValue, ScalarFunctionAbi,
    TargetAccessCallAssociation, TargetAccessCallBinding, TargetAccessInterpretation,
    TargetAccessViewInput, TargetBoundaryResult, TargetCallResult, TargetDescriptorOrigin,
    TargetNativeCallbackArgument, TargetStructuralResultContract, TargetUnitScalarArgumentSource,
    TargetUnitScalarCallArgument, selector_carrier_type,
};
pub mod case_slots;
pub use case_slots::TargetCasePointerSlot;
pub mod provenance;
pub use provenance::{
    CallSiteOwner, InstallationCallRole, ProgramEntryOrigin, TerminalPsiProvenance,
};
pub mod boundary;
mod descriptor_input;
pub use boundary::{
    AccessorScalarAccessRealization, BoundaryByteSequenceArgument, BoundaryExecutionBinding,
    BoundaryRealization, BoundaryScalarArgument, BoundarySettlementBinding,
    BoundarySettlementRealization, ClaimCompletionOnlyRealization, CompilerBuiltinExecution,
    DirectPortReadU8Realization, ExternalCompletionBinding, ExternalExecutionBinding,
    ForeignCallOperands, HostedExitProcessI32Realization, HostedReadByteRealization,
    HostedWriteByteI32Realization, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
    IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    LinuxWriteLineRealization, MetadataOnlyPortRealization, NativeBindingExecution,
    NearestIeeeFloatArithmeticRealization, NormalizedForeignCallBinding,
    NormalizedForeignCallMechanism, NormalizedForeignExecution, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, NormalizedSystemCallBinding, ProviderExecutionBinding,
    ProviderPlanReportIdentity, conventional_atomic_once_layout,
    conventional_atomic_once_layout_closed, conventional_float_class_layout,
};
pub use descriptor_input::{
    DescriptorInputGuard, descriptor_input_guard, descriptor_input_guard_for_call,
    descriptor_input_guard_for_call_with_closed_scalars,
    descriptor_input_guard_with_closed_scalars,
};
pub mod operation_records;
pub use operation_records::{
    NativeCallOrigin, TargetFloatResultPolicy, TargetInstallationCall,
    TargetStructuralRuntimeIndex, atomic_storage_integer_type, atomic_storage_integer_type_closed,
};
pub mod values;
pub use values::{
    ReferenceRoot, TargetBooleanExpression, TargetByteView, TargetElementView,
    TargetElementViewRegion, TargetIntegerExpression, TargetReferenceOrigin,
    TargetReferenceOriginBase, TargetReferenceResult, TargetScalarBlockValue,
    TargetScalarExpression, TargetScalarImmediate, TargetStructuralArgument,
    TargetStructuralArgumentSource, TargetStructuralParameter, TargetViewExtent,
};
pub mod storage;
pub use storage::{
    ScalarParameterLocation, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
    TargetStructuralHomeRequirement, TargetUnitScalarHomeRequirement,
    TargetUnitWriteOnlyPrimitiveStoreSource,
};
