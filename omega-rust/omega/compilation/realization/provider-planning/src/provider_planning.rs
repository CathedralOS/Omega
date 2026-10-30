//! Provider-plan derivation and selected source correspondence.
//!
//! Derive candidates from explicit satisfaction and prepare their exact source
//! associations. Whole-plan selection and complete binding belong to the
//! source-free provider-selection owner; Psi projects declaration evidence.
//!
//! This file derives plans and adapts their source provenance.
//! `selected_declarations.rs` projects selected source associations for source-free binding;
//! `source_provider_selections.rs` supplies early/reference source adapters;
//! `synchronous_cycles.rs` validates synchronous invocation
//! cycles, and `selection_provenance.rs` projects exact source correspondence
//! into the shared source-free `provider-selection` owner for whole-map choice
//! and independent-component admission;
//! Grant association consumes prepared issuer evidence through provider selection.
//! Product numeric satisfaction consumes Psi's checked declaration catalog.
//! Early Build's source-only requirement queries remain separate.

pub(crate) mod external_binding_rows;
pub(crate) mod native_catalogue_backing;
mod selected_declarations;
pub use selected_declarations::prepare_selected_plan_declarations;
mod provenance_replay;
pub(crate) mod provider_rows;

pub(crate) mod selection_declarations;
mod selection_provenance;
mod source_provider_selections;
mod synchronous_cycles;

pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
pub(crate) use external_binding_rows::extract_native_external_binding_rows;
pub(crate) use provenance_replay::{DerivedProviderPlan, ProviderPlanProvenance};
pub use provenance_replay::{
    ProviderPlanDerivation, ProviderRowRealization, ProviderSchemaDeclaration,
    derive_satisfies_plans, exact_checked_adapter, project_checked_provider_instances,
    same_semantic_name, satisfies_plan_name, selected_requirement_checked_adapter,
};

pub use selection_provenance::{
    ProviderSelectionProvenance, SelectedProviderReviewProvenance, selected_provider_plan_facts,
};
pub(crate) use selection_provenance::{
    SelectedProviderPlanWithProvenance, selected_provider_plan_facts_with_independent_components,
};
pub use source_provider_selections::{
    selected_checked_operator_body, selected_requirement_plan, selected_use_plan,
};
pub(crate) use synchronous_cycles::validate_selected_synchronous_invocation_cycles;

use checked_trees::typed_trees::TypedTrees;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine. Each selected plan keeps
/// its derivation provenance and the authority that selected it.
pub fn select_derived_provider_plans(
    derived: &[DerivedProviderPlan],
    selected_target: target::TargetProfile,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanWithProvenance>, Vec<diagnostics::Diagnostic>> {
    let candidates = derived
        .iter()
        .cloned()
        .map(selection_declarations::source_candidate)
        .collect();
    let defaults = defaults
        .iter()
        .map(selection_declarations::project_source_declaration)
        .collect::<Vec<_>>();
    let requested = requested
        .iter()
        .map(selection_declarations::project_source_declaration)
        .collect::<Vec<_>>();
    let selected = provider_selection::choose(candidates, selected_target, &defaults, &requested)?;
    Ok(selected
        .into_iter()
        .map(|selected| SelectedProviderPlanWithProvenance {
            derived: DerivedProviderPlan {
                plan: selected.candidate.plan,
                provenance: selected.candidate.owner,
            },
            selected_by: selection_declarations::restore_provenance(selected.selected_by),
        })
        .collect())
}
