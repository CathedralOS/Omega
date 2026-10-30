//! Reconcile exact external satisfaction and retained numeric execution meaning.

use effects::CompilerIntrinsicExecutionIdentity;
use effects::numeric_realizations::reconcile_numeric_meaning;
use effects::provider_plan::ProviderPlan;

/// Join the selected external leaf to Psi's exact satisfaction edge. Numeric
/// meaning alone cannot authorize execution; a same-signature provider or a
/// declaration from another package cannot replace the selected realization.
pub fn selected_numeric_execution(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    requirement: &terminal_psi::NominalDeclarationIdentity,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
    realization: Option<&str>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    let requirement = identities
        .text(requirement.identity())
        .ok_or("numeric provider requirement has no nominal identity")?;
    selected_execution_for_identity(
        numeric_realizations,
        identities,
        requirement,
        plan,
        row,
        retained,
        realization,
    )
}

pub(crate) fn selected_execution_for_identity(
    numeric_realizations: &[terminal_psi::NumericRealization],
    identities: &terminal_psi::TerminalIdentityTable,
    requirement: &str,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
    realization: Option<&str>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    let candidate = effects::numeric_realizations::selected_numeric_realization(
        numeric_realizations,
        identities,
        effects::numeric_realizations::NumericInvocation::Requirement { declaration: requirement, realization },
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
