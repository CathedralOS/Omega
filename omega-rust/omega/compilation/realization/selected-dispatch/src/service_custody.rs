//! Exact Fused routed-service establishment for a selected `ProgramEntry`
//! receiver.
//!
//! Psi records each logical `Binding<R>` carrier without selecting a provider.
//! This owner joins those occurrences to the selected Fused plans and emits
//! entry establishment; a logical marker alone grants no runtime authority.

mod root;

pub(crate) use root::derive_fused_program_entry_establishments;
pub use root::program_entry_service_requirements;

use checked_trees::{CheckedTrees, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape};
use diagnostics::Diagnostic;
use provider_planning::CompositionMode;
use provider_planning::SelectedProviderReviewProvenance;
use typed_trees::data::{DataDefinition, DataField};

/// Selection presence is used for discovery tolerance, never to mint custody.
/// Establishment separately joins the exact source schema and selected plan.
pub(crate) fn has_fused_selection(
    selected: &[SelectedProviderReviewProvenance],
    requirement: symbols::SymbolHandle,
) -> bool {
    selected.iter().any(|candidate| {
        candidate.provider.schema.symbol() == requirement
            && candidate.selected_by.composition_mode() == Ok(CompositionMode::Fused)
    })
}

fn data_field_identity(field: &DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}
