//! Closed declaration associations for already selected provider plans.
//!
//! These records contain no checked trees or source handles. Keeping each whole
//! plan with its selecting declaration distinguishes providers that share code.
//! Artifact-local family member identities are joined by native preparation;
//! neither this carrier nor constructing it grants execution authority.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProviderSources {
    pub declarations: Vec<crate::TerminalTargetProviderPlan>,
    pub family_realizations: Vec<SelectedFamilyRealizationSource>,
}

/// Exact source declaration association for one selected provider row. Only
/// the produced Terminal artifact can supply its physical member identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFamilyRealizationSource {
    pub head_declaration_identity: String,
    pub head_callable_identity: String,
    pub target: String,
    pub selected_requirement_identity: String,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
}
