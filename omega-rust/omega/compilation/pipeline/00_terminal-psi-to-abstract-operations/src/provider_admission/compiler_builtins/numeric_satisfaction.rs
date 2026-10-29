//! Join selected external realizations to canonical satisfaction and exact owners.

use effects::CompilerIntrinsicExecutionIdentity;
use effects::numeric_realizations::matches_numeric_selection;
pub(super) use effects::numeric_realizations::reconcile_numeric_meaning;
use effects::provider_plan::ProviderPlan;

/// Join the selected external leaf to Psi's exact satisfaction edge. Numeric
/// meaning alone cannot authorize execution; a same-signature provider or a
/// declaration from another package cannot replace the selected realization.
pub(crate) fn selected_execution(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    requirement: &terminal_psi::NominalDeclarationIdentity,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, &'static str> {
    let requirement = identities
        .text(requirement.identity())
        .ok_or("numeric provider requirement has no nominal identity")?;
    execution_for_invocation(
        numeric_realizations,
        identities,
        effects::numeric_realizations::NumericInvocation::Requirement {
            declaration: requirement,
            realization: None,
        },
        plan,
        row,
        retained,
    )
}

pub(crate) fn direct_execution(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    realization: &terminal_psi::NominalDeclarationIdentity,
    semantic_plan: &ProviderPlan,
    semantic_row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, &'static str> {
    let realization = identities
        .text(realization.identity())
        .ok_or("direct numeric invocation has no exact realization declaration")?;
    execution_for_invocation(
        numeric_realizations,
        identities,
        effects::numeric_realizations::NumericInvocation::Realization {
            declaration: realization,
        },
        semantic_plan,
        semantic_row,
        retained,
    )
}

fn execution_for_invocation(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    invocation: effects::numeric_realizations::NumericInvocation<'_>,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, &'static str> {
    let candidate = effects::numeric_realizations::selected_numeric_realization(
        numeric_realizations,
        identities,
        invocation,
        plan,
        row,
    )?;
    let Some(candidate) = candidate else {
        reconcile_numeric_meaning(None, retained)?;
        return Ok(retained);
    };
    candidate.validate(identities)?;
    let meaning = candidate.numeric_intrinsic;
    let execution = CompilerIntrinsicExecutionIdentity::from(meaning);
    // A source-review consumer can still supply its independently retained row.
    // It cannot replace the portable meaning. Native persistence omits it.
    if retained.is_some() {
        reconcile_numeric_meaning(Some(meaning), retained)?;
    }
    Ok(Some(execution))
}

/// Rejoin exact selecting owners at native proposal ingress, including private
/// record recovery. Later execution borrows this same immutable catalog and
/// selected plan; it does not need another parallel declaration sidecar.
pub(crate) fn validate_numeric_realization_owners(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), &'static str> {
    for selected in declarations {
        for row in &selected.plan.rows {
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } =
                &row.binding
            else {
                continue;
            };
            for candidate in numeric_realizations.iter().filter(|candidate| {
                matches_numeric_selection(candidate, identities, &selected.plan, row, machine)
            }) {
                let owner = candidate
                    .provider
                    .as_ref()
                    .and_then(|provider| identities.text(provider.declaration.identity()));
                if owner != selected.provider_declaration.as_deref() {
                    return Err("selected external provider has a different exact declaring owner");
                }
            }
        }
    }
    Ok(())
}
