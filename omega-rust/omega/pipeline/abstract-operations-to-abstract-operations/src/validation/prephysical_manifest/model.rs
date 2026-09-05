use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPrePhysicalOptimizationManifest {
    pub(super) record: PrePhysicalOptimizationManifest,
}

impl ValidatedPrePhysicalOptimizationManifest {
    pub const fn record(&self) -> &PrePhysicalOptimizationManifest {
        &self.record
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePhysicalOptimizationManifestError {
    InitialUnitProjection,
    StructuralStatisticsOverflow,
    ProjectionMismatch,
    SelectionMismatch,
    DecisionLogMismatch,
    LedgerMismatch,
    PassManifestCodecMismatch,
    PassRevisionMismatch,
    WorkUsageOverflow,
    WorkUsageMismatch,
    WorkBudgetExceeded,
    ContentMismatch,
}

impl std::fmt::Display for PrePhysicalOptimizationManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-physical optimization manifest: {self:?}"
        )
    }
}

impl std::error::Error for PrePhysicalOptimizationManifestError {}
