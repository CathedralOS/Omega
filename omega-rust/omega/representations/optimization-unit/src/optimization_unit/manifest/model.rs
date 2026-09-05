use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationManifestStage {
    /// Abstract-plan projection is independently validated. Target selection,
    /// allocation, frame/spill accounting, emission, and publication are not.
    PrePhysicalAbstractPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalOptimizationDataStatus {
    UnavailableBeforePhysicalRealization,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizationStructuralStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub nodes: u64,
    pub scalar_definitions: u64,
    pub scalar_uses: u64,
    pub optimization_facts: u64,
    pub ownership_frontier_facts: u64,
}

/// Structured, non-publication manifest for the largest independently
/// validated optimizer state available before target/physical realization.
///
/// Public fields make the record serializable and testable, but do not grant
/// authority. Downstream custody accepts only the validated wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrePhysicalOptimizationManifest {
    pub identity: PrePhysicalOptimizationManifestIdentity,
    pub stage: OptimizationManifestStage,
    pub physical_data: PhysicalOptimizationDataStatus,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub initial_unit: OptimizationUnitIdentity,
    pub final_unit: OptimizationUnitIdentity,
    pub projection: OptimizedAbstractPlanProjectionIdentity,
    /// Complete source-visible suite requested by the root build.
    pub selections: OptimizationSelections,
    /// Exact selection subset executed and validated in this Psi-stage record.
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub decision_log: BaselineDecisionLog,
    pub pass_manifests: Vec<OptimizationPassManifestRecord>,
    pub transformation_ledger: PsiTransformationLedger,
    pub identity_bundle: OptimizationIdentityBundle,
    pub source_statistics: OptimizationStructuralStatistics,
    pub optimized_statistics: OptimizationStructuralStatistics,
}

impl PrePhysicalOptimizationManifest {
    pub fn recomputed_identity(&self) -> PrePhysicalOptimizationManifestIdentity {
        super::codec::pre_physical_manifest_identity(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePhysicalOptimizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownPhysicalStatus(u8),
    UnsupportedVocabulary(u16),
    InvalidFuelSchedule,
    LengthOverflow,
    InvalidSelections,
    InvalidWorkBudget,
    InvalidWorkUsage,
    InvalidDecisionLog,
    InvalidPassManifest,
    InvalidTransformationLedger,
    InvalidIdentityBundle,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for PrePhysicalOptimizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-physical manifest encoding: {self:?}"
        )
    }
}

impl std::error::Error for PrePhysicalOptimizationManifestDecodeError {}
