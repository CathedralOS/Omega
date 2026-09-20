//! Float-meaning publication for the selected module.
//!
//! Each checked float-meaning projection first resolves the direct source
//! binding the route's exact projection sources attest, then lowers against
//! it. Resolved direct sources never emit their checked transitional
//! fallback, so surviving fallback identities renumber densely in emission
//! order; equalities lower without remapping.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;

use crate::lowering_error::LoweringError;
use crate::proofs::float_meaning_projection::{
    lower_float_meaning_equality, lower_float_meaning_projection,
    resolve_direct_float_source_binding,
};

/// Retain the checked float-meaning projections and equalities on the
/// selected module, renumbering transitional fallback inputs densely.
pub(crate) fn retain_float_meanings(
    checked: &CheckedTrees,
    projection_sources: &[(SymbolHandle, MachineId)],
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    lowered.semantic_module.float_meaning_projections = checked
        .facts
        .proof
        .float_meaning_projections
        .iter()
        .cloned()
        .map(|projection| {
            let direct_source = resolve_direct_float_source_binding(
                checked,
                projection_sources,
                &lowered.semantic_module.machines,
                &lowered.semantic_module.structural_types,
                &lowered.source_call_occurrences,
                &lowered.selected_ieee_float_fma_occurrences,
                projection.clone(),
            )?;
            lower_float_meaning_projection(projection, direct_source)
                .map_err(LoweringError::InvalidFloatMeaningProjection)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Resolved direct sources never emit their checked transitional fallback,
    // so surviving fallback identities renumber densely in emission order.
    let mut transitional_sources = Vec::<u32>::new();
    for projection in &mut lowered.semantic_module.float_meaning_projections {
        if let terminal_psi::FloatMeaningSource::TransitionalInput(input) = &mut projection.source {
            let next = match transitional_sources.iter().position(|id| *id == input.id.0) {
                Some(index) => index,
                None => {
                    transitional_sources.push(input.id.0);
                    transitional_sources.len() - 1
                }
            };
            input.id = terminal_psi::FloatProjectionInputId(u32::try_from(next).map_err(|_| {
                LoweringError::Unsupported(
                    "float-meaning transitional sources exceed their dense identity space",
                )
            })?);
        }
    }
    lowered.semantic_module.float_meaning_equalities = checked
        .facts
        .proof
        .float_meaning_equalities
        .iter()
        .copied()
        .map(lower_float_meaning_equality)
        .collect();
    Ok(())
}
