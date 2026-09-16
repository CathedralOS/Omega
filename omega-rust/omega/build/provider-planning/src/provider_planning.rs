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
//! cycles and `selection_provenance.rs` carries selection keys, provenance
//! and slot resolution; the remaining files carry bindings, reach,
//! intrinsics, provenance replay and receipts.

pub(crate) mod selection;

mod external_binding_rows;
mod installation_reach;
#[cfg(feature = "installed-writer")]
mod installed_writer;
mod intrinsic_execution;
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
};
pub use external_binding_rows::{
    extract_external_binding_rows, extract_native_external_binding_rows,
    settle_external_binding_rows,
};
#[cfg(feature = "installed-writer")]
pub use installed_writer::*;
pub use intrinsic_execution::primitive_float_binary_intrinsic_execution_identity;
pub use operator_provider_evidence::{
    compiler_intrinsic_diagnostic_label, intrinsic_realization_matches_operator,
};
pub use provenance_replay::*;
pub use selected_plan_bindings::SelectedProviderPlanBinding;
pub use selection_provenance::{
    ProviderSelectionProvenance, SelectedProviderPlanWithProvenance,
    SelectedProviderReviewProvenance, selected_provider_plan_facts_with_provenance,
};
pub use synchronous_cycles::validate_selected_synchronous_invocation_cycles;

use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use std::sync::Arc;
use trust_model::AuthoredRootGrant;
#[cfg(test)]
use trust_model::ProviderGrantSelectorKind;
use trust_model::resolve_selected_provider_grants;
use typed_trees::TypedTrees;

use crate::provider_planning::operator_provider_evidence::plan_selected_operator_provider_evidence;
use crate::provider_planning::selected_plan_bindings::SelectedProviderProgramUpdates;
use crate::provider_planning::selection_provenance::select_provider_plan_indices;
use installation_reach::derive_selected_installation_reach_resolutions;
#[cfg(test)]
use provenance_replay::exact_canonical_provider_schema;
#[cfg(feature = "installed-writer")]
use provenance_replay::same_semantic_name;

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
    let authored_provider_grants = trust_model::resolve_authored_selected_provider_grants(
        candidates,
        &facts,
        authored_root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let receipt_updates =
        receipt_binding::plan_admitted_receipt_updates(checked, &facts, &provider_grants)?;
    let (spelled_operator_uses, named_operator_uses) =
        plan_selected_operator_provider_evidence(checked, candidates, &facts)?;
    let installation_reach_resolutions =
        derive_selected_installation_reach_resolutions(checked, &facts)?;
    let selected = facts
        .with_installation_reach_resolutions(installation_reach_resolutions)
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
    let updates = SelectedProviderProgramUpdates {
        spelled_operator_uses,
        named_operator_uses,
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

/// PRV4 order step (2): derive plans from explicit SATISFIES edges -- one
/// plan per (provider type, boundary trait, target), assembled only from
/// that provider's conformance closure. External leaves and checked adapters
/// attached to the same provider type join one plan. External leaves may be
/// free declarations; checked adapters must belong to a nominal provider type
/// so execution can only dispatch through a retained whole-provider selection.
/// Coverage never combines unrelated provider types. Coverage/signatures come from the typed schema
/// (signature refinement is enforced by the conformance checker on each
/// edge); the effect surface is the union of the SATISFIED requirements'
/// declared effects -- the requirement supplies the ceiling, never the
/// leaf. Selection v1: a slot whose (trait, target) has exactly one FULLY
/// COVERING derived plan selects it implicitly; ambiguity or partial
/// coverage is loud at the consumer (the trust report shows coverage).
pub fn derive_satisfies_plans(
    typed: &TypedTrees,
    selected_target: Option<&str>,
) -> Vec<ProviderPlan> {
    derive_satisfies_plans_with_provenance(typed, selected_target)
        .into_iter()
        .map(|derived| derived.plan)
        .collect()
}

/// PRV4c: select one fully covering provider type per applicable boundary
/// slot. An explicit build-root declaration wins over the selected target
/// package's ordinary default declaration. Without either, a unique covering
/// candidate supplies the declaration-era default. Rows are never selected
/// individually and partial candidates never combine.
pub fn select_provider_plans(
    plans: &[effects::provider_plan::ProviderPlan],
    selected_target: target::NativeTarget,
    defaults: &[crate::ProviderSelection],
    requested: &[crate::ProviderSelection],
) -> Result<Vec<ProviderPlan>, Vec<diagnostics::Diagnostic>> {
    select_provider_plan_indices(plans, selected_target, defaults, requested).map(|selected| {
        selected
            .into_iter()
            .map(|selected| plans[selected.candidate].clone())
            .collect()
    })
}

pub fn select_provider_plans_with_provenance(
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
