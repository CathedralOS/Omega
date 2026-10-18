//! Resolve checked expression-owner disposal against emitted storage.
//!
//! Source checking has already selected and ordered disposal, including each
//! hook's exact consumed claims. Emission joins those receipts to actual homes;
//! it never reconstructs permission or claim consumption from the owner type.

use super::{CleanupRegion, LoweringError, unsupported};
use crate::expression_preparation::bindings::{StructuralLocalBinding, structural_local_by_root};
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;

pub(crate) fn expression_cleanup_regions<'action>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    actions: impl IntoIterator<Item = &'action checked_trees::CheckedNormalCleanupAction>,
    completed_homes: &[StructuralLocalBinding],
    carrier_multiplicity: impl FnMut(
        semantic_vocabulary::PlaceId,
    ) -> Result<terminal_psi::StructuralMultiplicity, LoweringError>,
    calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let types = calls.structural_types;
    let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
    let actions = super::prepare_checked_cleanup_regions(
        identities,
        checked,
        actions,
        &[],
        control,
        types,
        |identities, identity| {
            Ok(
                crate::terminal_identities::structural_declaration_by_identity(
                    types,
                    identities,
                    checked,
                    identity,
                    "value completion has no structural type",
                )?
                .id,
            )
        },
        |identities, source, path| {
            let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
                source
            else {
                return unsupported("value completion lost its expression owner");
            };
            let binding = structural_local_by_root(
                completed_homes,
                facts::PlaceRoot::Expression(*expression),
            )
            .ok_or(LoweringError::Unsupported(
                "value completion lost its completed expression owner",
            ))?;
            if !binding.argument.path.is_empty() {
                return unsupported("value completion names an alias instead of its owned home");
            }
            let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                identities, path, binding.structural_type, types,
                None, &mut std::iter::empty(), None,
            )?;
            Ok((binding.argument.place, path))
        },
        carrier_multiplicity,
        |cleanup, _, _| Ok(cleanup.consumed_claims.clone()),
    )?;
    super::emit_prepared_cleanup_regions(&actions, calls)
}
