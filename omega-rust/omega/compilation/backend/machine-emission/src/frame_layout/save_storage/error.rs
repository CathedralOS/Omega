use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::RegisterUnitId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonAuthoritativeCalleeSaveStorageError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTargetCatalog,
    UnknownPreservedUnit(RegisterUnitId),
    NonCanonicalStorage,
    UsageMismatch,
    StorageGeometryOverflow,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
}

diagnostics::failed_display!(
    NonAuthoritativeCalleeSaveStorageError,
    "non-authoritative callee-save storage planning failed"
);

impl std::error::Error for NonAuthoritativeCalleeSaveStorageError {}
