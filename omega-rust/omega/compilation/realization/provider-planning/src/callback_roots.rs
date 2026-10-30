//! Join selected provider evidence to canonical callback-use declarations.
//!
//! This is the remaining source adapter, not ABI placement. Empty use rosters
//! do no instance work; otherwise the whole selected instance projection must
//! succeed before per-use declaration or calling-plan errors are considered.
//! Psi has already frozen classification, exact requirement coordinates and
//! body identities; this adapter never reconstructs them from checked uses.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use language_semantics::callback_declarations::CallbackRegistrationDeclaration;
use provider_selection::{CallbackRootCandidate, SelectedCallbackRow};

pub fn prepare(
    program: &CheckedTrees,
    selected_providers: &effects::SelectedProviderPlanFacts,
    declarations: &terminal_psi::CallbackDeclarationRoster,
    calling_requirements: &[terminal_psi::CallbackRequirementId],
) -> Result<Vec<terminal_psi::CallbackUseId>, Vec<Diagnostic>> {
    if declarations.uses.is_empty() {
        return Ok(Vec::new());
    }
    // This existing projection admits every selected checked adapter, not only
    // callback-reached rows. Retain its aggregate errors before per-use policy.
    let selected_instances = crate::project_checked_provider_instances(
        &program.typed,
        &program.facts,
        selected_providers,
    )?;
    let context =
        crate::ProviderPlanningContext::new(&program.typed, source::DependencyScope::Product);
    let candidates = declarations.uses.iter().map(|(usage_id, usage)| {
        let declaration = declarations.declaration(usage);
        let provider = match &declaration.registration {
            Ok(CallbackRegistrationDeclaration::External(identity)) => {
                prepare_selected_row(context, &selected_instances, identity)
            }
            _ => None,
        };
        let calling_plan_matches = calling_requirements
            .iter()
            .filter(|requirement| **requirement == usage.requirement)
            .count();
        CallbackRootCandidate {
            owner: usage_id,
            declaration,
            provider,
            calling_plan_matches,
        }
    });
    provider_selection::select_callback_roots(candidates)
}

fn prepare_selected_row<'plan>(
    context: crate::ProviderPlanningContext<'_>,
    selected_instances: &'plan effects::SelectedProviderPlanFacts,
    identity: &str,
) -> Option<SelectedCallbackRow<'plan>> {
    let (plan, row) = selected_instances.selected_requirement_row(identity)?;
    // Keep evidence attached to this full selected row. Errors remain data
    // until policy consumes a checked adapter, not an early batch refusal.
    let checked_adapter_body = match row.binding {
        ProviderBinding::CheckedAdapter { .. } => crate::exact_checked_adapter(context, plan, row)
            .map(|adapter| adapter.supplies_checked_body())
            .map_err(|diagnostic| {
                format!(
                    "callback forwarding provider is not its exact checked adapter: {diagnostic:?}"
                )
            }),
        _ => Ok(false),
    };
    Some(SelectedCallbackRow {
        plan,
        row,
        checked_adapter_body,
    })
}
