//! Optimizer module role: stage group. boundary in target operations.
//!
//! These modules own the related program facts; lowering algorithms live in
//! pipeline stages and consume these data types.

mod execution;
pub use execution::{
    BoundaryExecutionBinding, CompilerBuiltinExecution, ProviderExecutionBinding,
    ProviderPlanReportIdentity,
};
mod realizations;
pub use realizations::{
    BoundaryRealization, BoundarySettlementRealization, ClaimCompletionOnlyRealization,
    DirectPortReadU8Realization, HostedExitProcessI32Realization, HostedReadByteRealization,
    HostedWriteByteI32Realization, LinuxWriteLineRealization, MetadataOnlyPortRealization,
    NormalizedForeignCallBinding, NormalizedForeignScalarArgument,
    NormalizedForeignStructuralArgument,
};
mod arguments;
pub use arguments::{
    BoundaryByteSequenceArgument, BoundaryScalarArgument, BoundarySettlementBinding,
};
mod fma;
pub use fma::{TargetIeeeFloatFmaOperand, TargetX86ScalarFmaSettlement};
