/*
We define the target-provider, callback-placement, and direct-boundary plan
records that bridge canonical Terminal Psi artifacts into native realization.
Our carriers pair produced Terminal bodies and whole provider selections with
their target-owned ABI placements, declaration coordinates, and synchronous
invocation graphs.
*/

/*
We keep these realization plans separate from both source checking and native
execution admission so neither stage has to rediscover declaration joins or
invent a second scheduling pipeline. By retaining exact declaration custody
beside canonical artifacts, we distinguish providers and callback thunks that
share underlying machine code without granting host execution authority.
*/

/*
We consume canonical Terminal modules, calling-convention entry plans, and
provider-plan schemas produced upstream, and we expose closed plan proposals
and settlement projections to native preparation and installation admission.
*/

mod callback_artifacts;
mod direct_boundary_leaves;
pub mod synchronous_invocations;
mod target_providers;

pub use callback_artifacts::{
    BoundCallbackPrivateMaterialization, NativeCallbackEntryParameter, NativeCallbackPlacement,
    NativeCallbackThunkSettlement, PrivateCallbackDestinationApplication,
    TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact, native_callback_thunks,
    validate_callback_thunk_joins,
};
pub use direct_boundary_leaves::{DirectBoundaryExecution, DirectBoundaryLeaf};
pub use target_providers::{
    SelectedCheckedInvocationRow, SelectedProviderCandidateRow, TerminalTargetProviderPlan,
    TerminalTargetProviderProposal, TerminalTargetProviderSelection,
    admit_selected_checked_providers, close_checked_provider_specializations,
    matching_folded_constant_dependencies, require_selected_provider_declarations,
    selected_checked_adapter_header, selected_checked_invocation_rows,
    selected_provider_candidate_rows,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProviderSources {
    pub declarations: std::sync::Arc<[crate::TerminalTargetProviderPlan]>,
    pub family_realizations: Vec<SelectedFamilyRealizationSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFamilyRealizationSource {
    pub head_declaration_identity: String,
    pub head_callable_identity: String,
    pub target: String,
    pub selected_requirement_identity: String,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
}
