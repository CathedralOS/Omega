//! Replay the complete evaluated-import roster before native realization.
//!
//! This joins selected plans to their retained external bindings; it grants no
//! execution, calling-contract or stack authority. Those checks remain in the
//! ordinary provider admission path.

use calling_conventions::{ExternalBindingKind, ExternalBindingRow};
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use std::collections::BTreeSet;
use target::TargetProfile;

/// Require a one-to-one join for every selected evaluated import, including
/// imports not demanded by the current entry. Non-import rows are outside this
/// check; unmatched imported rows, reused rows and locator substitution reject.
/// Source evaluation has already normalized each complete locator.
pub fn validate_evaluated_import_rows(
    target_profile: TargetProfile,
    plans: &[ProviderPlan],
    external_binding_rows: &[ExternalBindingRow],
) -> Result<(), &'static str> {
    let target_name = target_profile.target_name();
    let mut matched_external_rows = BTreeSet::new();
    for plan in plans {
        let effective_target = if plan.target.is_empty() {
            target_name
        } else {
            plan.target.as_str()
        };
        for selected_row in &plan.rows {
            let ProviderBinding::Import { evaluated } = &selected_row.binding else {
                continue;
            };
            if effective_target != target_name || evaluated.locator().target() != target_profile {
                return Err(
                    "Terminal native proposal selected import disagrees with its target profile",
                );
            }
            let matching = external_binding_rows
                .iter()
                .enumerate()
                .filter(|(_, external)| {
                    external.target_name == effective_target
                        && external.trait_name == plan.schema.trait_name
                        && external.method == selected_row.method
                        && external.requirement_identity == selected_row.requirement_identity
                        && external.table_type == plan.provider_type
                })
                .collect::<Vec<_>>();
            let [(external_index, external)] = matching.as_slice() else {
                return Err(
                    "Terminal native proposal selected import does not rejoin one exact external-binding row",
                );
            };
            let ExternalBindingKind::Import { locator } = &external.binding else {
                return Err(
                    "Terminal native proposal selected import rejoined a non-evaluated external binding",
                );
            };
            if locator != evaluated.locator() || locator.target() != target_profile {
                return Err(
                    "Terminal native proposal external import substituted the selected evaluated locator",
                );
            }
            if !matched_external_rows.insert(*external_index) {
                return Err(
                    "Terminal native proposal external import was reused by multiple selected rows",
                );
            }
        }
    }
    if external_binding_rows
        .iter()
        .enumerate()
        .any(|(index, external)| {
            matches!(external.binding, ExternalBindingKind::Import { .. })
                && !matched_external_rows.contains(&index)
        })
    {
        return Err("Terminal native proposal retains an unmatched evaluated external import row");
    }
    Ok(())
}
