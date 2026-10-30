//! Validate exact numeric realization owners at native ingress.

use effects::numeric_realizations::matches_numeric_selection;
pub(super) use effects::numeric_realizations::reconcile_numeric_meaning;
pub(crate) use provider_selection::selected_numeric_execution as selected_execution;

/// Rejoin exact selecting owners at native proposal ingress, including private
/// record recovery. Later execution borrows this same immutable catalog and
/// selected plan; it does not need another parallel declaration sidecar.
pub(crate) fn validate_numeric_realization_owners(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), &'static str> {
    for selected in declarations {
        if selected.row_numeric_realizations.len() != selected.plan.rows.len() {
            return Err("selected external declarations have incomplete row custody");
        }
        for (row, realization) in selected
            .plan
            .rows
            .iter()
            .zip(&selected.row_numeric_realizations)
        {
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } =
                &row.binding
            else {
                if realization.is_some() {
                    return Err("non-intrinsic row substituted an external realization");
                }
                continue;
            };
            if realization.as_ref().is_some_and(String::is_empty) {
                return Err("selected external realization has empty declaration identity");
            }
            let mut found = false;
            for candidate in numeric_realizations.iter().filter(|candidate| {
                matches_numeric_selection(candidate, identities, &selected.plan, row, machine)
                    && realization.as_ref().is_none_or(|realization| {
                        identities.text(candidate.realization.machine.identity())
                            == Some(realization.as_str())
                    })
            }) {
                found = true;
                let owner = candidate
                    .provider
                    .as_ref()
                    .and_then(|provider| identities.text(provider.declaration.identity()));
                if owner != selected.provider_declaration.as_deref() {
                    return Err("selected external provider has a different exact declaring owner");
                }
            }
            if realization.is_some() && !found {
                return Err("selected external realization has no matching canonical satisfaction");
            }
        }
    }
    Ok(())
}
