use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::RegisterUnitId;

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

impl std::fmt::Display for NonAuthoritativeCalleeSaveStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "non-authoritative callee-save storage planning failed: {self:?}"
        )
    }
}

impl std::error::Error for NonAuthoritativeCalleeSaveStorageError {}
