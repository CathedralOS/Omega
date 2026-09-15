/// Checked occurrence before emission assigns real Terminal identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectedComparison {
    pub operator_use: checked_trees::CheckedOperatorUseHandle,
    pub application_site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    pub requirement_operator: symbols::SymbolHandle,
    pub provider_plan_report_fingerprint: u64,
    pub provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
    pub comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}
