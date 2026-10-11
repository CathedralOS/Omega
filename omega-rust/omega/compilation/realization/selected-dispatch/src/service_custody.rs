//! Exact Fused routed-service establishment for a selected `ProgramEntry`
//! receiver.
//!
//! Psi records each logical `Binding<R>` carrier without selecting a provider.
//! This owner joins those occurrences to the selected Fused plans and emits
//! entry establishment; a logical marker alone grants no runtime authority.

mod root;

pub(crate) use root::derive_fused_program_entry_establishments;
pub(crate) use root::program_entry_has_unsettled_service_fields;
pub use root::program_entry_service_requirements;

use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
use diagnostics::Diagnostic;
use provider_planning::CompositionMode;
use provider_planning::SelectedProviderReviewProvenance;
use typed_trees::data::{DataDefinition, DataField};

/// Selection presence is used for discovery tolerance, never to mint custody.
/// It retains the complete application even for an ancestor view beneath a
/// selected descendant. Establishment separately retains that whole plan.
pub(crate) fn has_fused_selection(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    carrier: typed_trees::service::ExactServiceCarrier,
) -> bool {
    let Some(schema) = carrier_schema(checked, &[], carrier) else {
        return false;
    };
    selected.iter().any(|candidate| {
        candidate.plan.schema.supplies_receiver_schema(&schema)
            && candidate.selected_by.composition_mode() == Ok(CompositionMode::Fused)
    })
}

fn carrier_schema(
    checked: &CheckedTrees,
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    carrier: typed_trees::service::ExactServiceCarrier,
) -> Option<effects::provider_plan::ServiceSchema> {
    provider_planning::service_schema::from_typed_application(
        provider_planning::ProviderPlanningContext::new(checked, source::DependencyScope::Product),
        calling_plans,
        carrier.application(checked)?,
        carrier.arguments(checked),
    )
}

fn data_field_identity(field: &DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}
