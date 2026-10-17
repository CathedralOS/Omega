//! Provider-plan selection and binding to checked programs.
//!
//! Derive candidates from explicit satisfaction, select exact slots, then bind
//! grants, qualification associations, operator evidence and installation reach.
//! Subordinate modules own validation and projection; consumers use the selected
//! immutable carrier rather than rediscovering declarations.
//!
//! This file binds, derives and selects provider plans.
//! `selected_plan_bindings.rs` carries selected facts and grants;
//! `operator_provider_evidence.rs` plans selected operator provider
//! evidence, `synchronous_cycles.rs` validates synchronous invocation
//! cycles, `selection_provenance.rs` carries selection keys, provenance
//! and slot resolution, and `independent_components.rs` joins each
//! `Independent` selection to exactly one verified component description;
//! `qualification_grants.rs` checks exact source requirements against grants.
//! Intrinsic execution identities and labels are queries on
//! `crate::IntrinsicRequirement`.

pub(crate) mod direct_foreign_leaves;
mod external_binding_rows;
mod external_requirements;
mod independent_components;
mod installation_reach;
mod operator_provider_evidence;
mod provenance_replay;
mod qualification_grants;
mod selected_plan_bindings;
mod selection_provenance;
mod synchronous_cycles;

pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
pub(crate) use external_binding_rows::extract_native_external_binding_rows;
pub use operator_provider_evidence::{
    selected_requirement_plan, selected_use_plan, specialized_provider_machines,
};
pub(crate) use provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, SelectedTargetMachineOrigin,
};
pub use provenance_replay::{
    ProviderPlanDerivation, ProviderSchemaDeclaration, derive_satisfies_plans,
    exact_checked_adapter, same_semantic_name, satisfies_plan_name,
    selected_requirement_checked_adapter,
};
pub(crate) use selected_plan_bindings::SelectedProviderPlanBinding;
pub use selection_provenance::{
    ProviderSelectionProvenance, SelectedProviderReviewProvenance, selected_provider_plan_facts,
};
pub(crate) use selection_provenance::{
    SelectedProviderPlanWithProvenance, selected_provider_plan_facts_with_independent_components,
};
pub(crate) use synchronous_cycles::validate_selected_synchronous_invocation_cycles;

use checked_trees::typed_trees::TypedTrees;
use effects::provider_grants::AuthoredRootGrant;
use effects::provider_grants::resolve_selected_provider_grants;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};

use crate::provider_planning::selection_provenance::select_provider_plan_indices;
use installation_reach::derive_selected_installation_reach_resolutions;

/// Build the exact Omega-owned selection sidecar without changing checked Psi.
/// Provider execution and compiler-generated helpers consume this carrier; neither
/// may reconstruct a plan by scanning authored `satisfies` rows.
pub fn bind_selected_provider_plan_facts(
    typed: &checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    candidates: &[ProviderPlan],
    facts: effects::SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
    root_grants: &[String],
    authored_root_grants: &[AuthoredRootGrant],
) -> Result<SelectedProviderPlanBinding, Vec<diagnostics::Diagnostic>> {
    let provider_grants = resolve_selected_provider_grants(candidates, &facts, root_grants)
        .map_err(|diagnostic| vec![diagnostic])?;
    let authored_provider_grants =
        effects::provider_grants::resolve_authored_selected_provider_grants(
            candidates,
            &facts,
            authored_root_grants,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    qualification_grants::validate(typed, check_facts, &facts, &provider_grants)?;
    operator_provider_evidence::validate_selected_operator_provider_evidence(
        typed,
        check_facts,
        candidates,
        &facts,
    )?;
    operator_provider_evidence::validate_selected_requirement_provider_evidence(
        typed,
        check_facts,
        candidates,
        &facts,
    )?;
    let (installation_reach_resolutions, installation_reach_identities) =
        derive_selected_installation_reach_resolutions(typed, check_facts, &facts)?;
    let (specialized_requirement_rows, specialized_identities) =
        provenance_replay::derive_selected_requirement_specializations(typed, check_facts, &facts)?;
    let (external_applications, external_identities) =
        external_requirements::derive(typed, &facts, provenance)
            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    let selected = facts
        .with_specialized_requirement_rows(specialized_requirement_rows, &specialized_identities)
        .and_then(|facts| {
            facts
                .with_external_requirement_applications(external_applications, &external_identities)
        })
        .and_then(|facts| {
            facts.with_installation_reach_resolutions(
                installation_reach_resolutions,
                &installation_reach_identities,
            )
        })
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    Ok(SelectedProviderPlanBinding {
        selected,
        grants: authored_provider_grants,
    })
}

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine. Each selected plan keeps
/// its derivation provenance and the authority that selected it.
pub fn select_derived_provider_plans(
    derived: &[DerivedProviderPlan],
    selected_target: target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<SelectedProviderPlanWithProvenance>, Vec<diagnostics::Diagnostic>> {
    let plans = derived
        .iter()
        .map(|derived| derived.plan.clone())
        .collect::<Vec<_>>();
    select_provider_plan_indices(&plans, selected_target, defaults, requested).map(|selected| {
        selected
            .into_iter()
            .map(|selected| SelectedProviderPlanWithProvenance {
                derived: derived[selected.candidate].clone(),
                selected_by: selected.selected_by,
            })
            .collect()
    })
}
