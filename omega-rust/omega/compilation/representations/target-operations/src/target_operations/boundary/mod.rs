//! Optimizer module role: stage group. boundary in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod execution;
pub use execution::{
    BoundaryExecutionBinding, CompilerBuiltinExecution, IeeeFloatArithmeticOperation,
    IeeeFloatBinaryOperation, IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection,
    IeeeFloatToIntegerDomain, ProviderExecutionBinding, ProviderPlanReportIdentity,
};
mod realizations;
pub use realizations::{
    BoundaryRealization, BoundarySettlementRealization, ClaimCompletionOnlyRealization,
    DirectPortReadU8Realization, ForeignCallOperands, HostedExitProcessI32Realization,
    HostedReadByteRealization, HostedWriteByteI32Realization, LinuxWriteLineRealization,
    MetadataOnlyPortRealization, NearestIeeeFloatArithmeticRealization,
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument, NormalizedSystemCallBinding,
    PlacedScalarAccessRealization,
};
mod arguments;
pub use arguments::{
    BoundaryByteSequenceArgument, BoundaryScalarArgument, BoundarySettlementBinding,
};
mod fma;
pub use fma::TargetIeeeFloatFmaOperand;
pub(crate) use fma::TargetX86ScalarFmaSettlement;

mod float_class;
pub use float_class::conventional_float_class_layout;
