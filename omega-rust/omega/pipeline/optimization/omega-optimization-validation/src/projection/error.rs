//! Optimized-plan projection rejection vocabulary.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAbstractPlanProjectionError {
    FinalUnit(OptimizationUnitValidationError),
    InitialUnitProjection,
    LedgerReplay(InvalidPsiTransformationLedger),
    LedgerTerminalMismatch,
    LedgerFuelMismatch,
    LedgerInitialMismatch,
    LedgerFinalMismatch,
    SelectionIdentityMismatch,
    PsiSelectionProjectionMismatch,
    RuleSetIdentityMismatch,
    CostModelIdentityMismatch,
    DecisionLogIdentityMismatch,
    DecisionLogReplay(BaselineDecisionLogDecodeError),
    WorkloadProfileNotSupported,
    LedgerIdentityMismatch,
    ManifestPresenceMismatch,
    ManifestCodecMismatch,
    ManifestRevisionMismatch,
    ManifestRuleSetMismatch,
    ManifestLedgerMismatch,
    SourceCustodyMismatch,
    SourceFunctionRosterMismatch,
    ImmutablePlanMetadataMismatch,
    ReconstructibleProjectionMismatch,
}

impl std::fmt::Display for OptimizedAbstractPlanProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized abstract-plan projection: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAbstractPlanProjectionError {}
