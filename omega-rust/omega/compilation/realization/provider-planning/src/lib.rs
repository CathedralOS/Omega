#![forbid(unsafe_code)]

/*
We derive and realize Omega provider plans from checked source declarations,
calling-policy evaluations, component progress contracts, and task activation
plans. We receive checked inputs from the compiler coordinator and return
target-specific provider plans without embedding target policy or layout choices
inside the coordinator itself.

We resolve exact execution grants from backend provider evidence independently of
compiler trust reporting. Checked-tree and package-input custody dependencies
serve as our realization bridge: typed vocabulary enters this crate exclusively
through `checked_trees` re-exports, and `source_guards` rejects any direct
source-input dependency edge or facade-free typed spelling.

Start at `provider_settlement.rs` for typed candidate derivation and selection,
then `provider_planning.rs` for selected source correspondence. Whole-map choice,
selected-plan binding, and composition admission live in `provider-selection`,
while we supply exact invocation correspondence for checking and review alongside
canonical declaration records for artifact publication.
*/

pub mod calling_policy_plans;
pub mod data_layout_queries;
pub mod dispatch_tables;
pub mod native_bindings;
pub(crate) use optimization_core::digest_encoder;
pub mod evaluated_via_bindings;
mod intrinsic_requirements;
mod native_provider_sources;
pub mod provider_declarations;
mod sealed_accessors;
pub use sealed_accessors::{SealedAccessor, SealedAccessorScalarOperation};
pub mod program_entry;
mod provider_identity;
pub use provider_identity::{
    provider_row_matches, provider_trait_application, provider_trait_requirement_matches,
};
pub mod intrinsic_declarations;
mod provider_planning;
mod provider_settlement;
pub mod service_declarations;
pub mod service_schema;
#[cfg(test)]
mod source_guards;
pub mod target_machines;

pub use provider_settlement::SettledTargetMachineDeclarations;
pub use provider_settlement::{
    CheckedProviderSelection, ComponentAssumptionDemand, ProviderSettlement,
    settle_checked_providers, verify_selected_compositions_are_realized,
};

pub use build_time_evaluation::provider_declarations::{
    ProviderSelection, ProviderSelectionSubject, TargetProviderDefaults,
};
pub use checked_trees::typed_trees::provider_declarations::ProviderPlanningContext;
pub use intrinsic_requirements::{
    IntrinsicRequirement, IntrinsicRequirementKind, primitive_numeric_type,
};
pub use language_semantics::declaration_selection::CompositionMode;
pub use provider_planning::extract_native_external_binding_rows;
pub use provider_planning::provider_rows::{
    DeferredProviderRows, DirectBoundaryInput, DirectBoundaryMechanism, DirectBoundarySource,
    PreparedProviderRows, ProviderPlanCandidate, ProviderRowDeclarations,
};
pub use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation, ProviderPlanDerivation, ProviderSchemaDeclaration,
    ProviderSelectionProvenance, SelectedProviderReviewProvenance, derive_satisfies_plans,
    exact_checked_adapter, prepare_selected_build_time_provider_body,
    prepare_selected_plan_declarations, same_semantic_name, satisfies_plan_name,
    select_derived_provider_plans, selected_checked_operator_body, selected_provider_plan_facts,
    selected_requirement_checked_adapter, selected_requirement_plan, selected_use_plan,
};
pub(crate) use provider_planning::{
    DerivedProviderPlan, ProviderPlanProvenance, SelectedTargetMachineOrigin,
};

pub use provider_planning::ProviderRowRealization;
