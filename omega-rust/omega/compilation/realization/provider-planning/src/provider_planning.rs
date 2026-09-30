//! Provider-plan selection and binding to checked programs.
//!
//! Derive candidates from explicit satisfaction, select exact slots, then bind
//! grants, receipts, operator evidence, and installation reach before publishing.
//! Subordinate modules own validation and projection; consumers use the selected
//! immutable carrier rather than rediscovering declarations.
//!
//! This file binds, derives and selects provider plans.
//! `selected_plan_bindings.rs` carries the selected plan binding and program
//! updates, `operator_provider_evidence.rs` plans selected operator provider
//! evidence, `synchronous_cycles.rs` validates synchronous invocation
//! cycles, `selection_provenance.rs` carries selection keys, provenance
//! and slot resolution, and `independent_components.rs` joins each
//! `Independent` selection to exactly one verified component description;
//! the remaining files carry bindings, reach, provenance replay and receipts.
//! Intrinsic execution identities and labels are queries on
//! `crate::IntrinsicRequirement`.

pub(crate) mod selection;

mod external_binding_rows;
mod independent_components;
mod installation_reach;
mod operator_provider_evidence;
mod provenance_replay;
mod receipt_binding;
mod selected_plan_bindings;
mod selection_provenance;
mod synchronous_cycles;
#[cfg(test)]
mod tests;

pub use effects::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
pub(crate) use external_binding_rows::extract_native_external_binding_rows;
pub use operator_provider_evidence::{selected_requirement_plan, selected_use_plan};
#[cfg(test)]
pub(crate) use provenance_replay::exact_checked_adapter_invocations;
#[cfg(test)]
pub(crate) use provenance_replay::validate_provider_plan_candidates;
pub use provenance_replay::{
    DerivedProviderPlan, ProviderPlanDerivation, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, derive_satisfies_plans, exact_checked_adapter, same_semantic_name,
    satisfies_plan_name, selected_requirement_checked_adapter,
    validate_derived_provider_plan_candidates,
};
pub use selected_plan_bindings::SelectedProviderPlanBinding;
pub use selection_provenance::{
    ProviderSelectionProvenance, SelectedProviderPlanWithProvenance,
    SelectedProviderReviewProvenance, selected_provider_plan_facts,
    selected_provider_plan_facts_with_independent_components,
};
pub(crate) use synchronous_cycles::validate_selected_synchronous_invocation_cycles;

use effects::provider_grants::AuthoredRootGrant;
use effects::provider_grants::resolve_selected_provider_grants;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use std::sync::Arc;
use typed_trees::TypedTrees;

use crate::provider_planning::selected_plan_bindings::SelectedProviderProgramUpdates;
use crate::provider_planning::selection_provenance::select_provider_plan_indices;
use installation_reach::derive_selected_installation_reach_resolutions;
#[cfg(test)]
use provenance_replay::exact_canonical_provider_schema;

/// Build the exact Omega-owned selection sidecar and bind its stable receipt
/// identities into checked semantic evidence. Provider execution and
/// compiler-generated helper machines consume the returned carrier; neither
/// may reconstruct a plan by scanning authored `satisfies` rows.
pub fn bind_selected_provider_plan_facts(
    program: &Arc<checked_trees::CheckedTrees>,
    candidates: &[ProviderPlan],
    facts: effects::SelectedProviderPlanFacts,
    root_grants: &[String],
    authored_root_grants: &[AuthoredRootGrant],
) -> Result<SelectedProviderPlanBinding, Vec<diagnostics::Diagnostic>> {
    let checked = program.as_ref();
    let provider_grants = resolve_selected_provider_grants(candidates, &facts, root_grants)
        .map_err(|diagnostic| vec![diagnostic])?;
    let authored_provider_grants =
        effects::provider_grants::resolve_authored_selected_provider_grants(
            candidates,
            &facts,
            authored_root_grants,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    let receipt_updates =
        receipt_binding::plan_admitted_receipt_updates(checked, &facts, &provider_grants)?;
    operator_provider_evidence::validate_selected_operator_provider_evidence(
        checked, candidates, &facts,
    )?;
    operator_provider_evidence::validate_selected_requirement_provider_evidence(
        checked, candidates, &facts,
    )?;
    let installation_reach_resolutions =
        derive_selected_installation_reach_resolutions(checked, &facts)?;
    let specialized_requirement_rows =
        provenance_replay::derive_selected_requirement_specializations(checked, &facts)?;
    let selected = facts
        .with_specialized_requirement_rows(specialized_requirement_rows)
        .and_then(|facts| facts.with_installation_reach_resolutions(installation_reach_resolutions))
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    let updates = SelectedProviderProgramUpdates {
        admitted_receipts: receipt_updates,
    };
    let mut bound_program = Arc::clone(program);
    if !updates.is_empty() {
        updates.apply(Arc::make_mut(&mut bound_program));
    }
    Ok(SelectedProviderPlanBinding {
        program: bound_program,
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
