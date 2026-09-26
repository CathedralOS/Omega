//! Execution roles, settlement occurrences, and privileged effects.

pub mod execution;
pub mod ports;
pub mod settlements;

pub use execution::{
    BoundaryExecutionRecord, CompletionProviderCustodyBinding, ProviderExecutionRecord,
    derive_completion_provider_custody,
};
pub use ports::PortEffectRecord;
pub use settlements::{
    BoundaryByteSequenceArgumentRecord, BoundaryResultRecord, BoundaryScalarResultRecord,
    BoundarySettlementRecord, BoundaryStructuralResultRecord,
};
