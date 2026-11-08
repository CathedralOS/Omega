// Declaration-identity closure for selected provider implementations. Closes
// the selecting nominal map and row-aligned numeric-intrinsic realization
// identities while source custody is available so Build interpretation and
// native publication can join those records to the Terminal artifact without
// granting execution authority.

use crate::{ProviderPlanningContext, SelectedProviderReviewProvenance};
use backend_plan::TerminalTargetProviderPlan;
use diagnostics::Diagnostic;

pub(crate) fn close_selecting_declaration(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    provider: &crate::ProviderPlanProvenance,
) -> Result<Option<String>, Vec<Diagnostic>> {
    provider
        .provider_type
        .map(|symbol| close_nominal_declaration(context, exact_toolchain_sources, symbol))
        .transpose()
}

fn close_nominal_declaration(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    symbol: symbols::SymbolHandle,
) -> Result<String, Vec<Diagnostic>> {
    context
        .typed
        .exact_owner_nominal_identity(symbol, exact_toolchain_sources)
        .and_then(|identity| context.typed.type_identity_text(identity))
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected provider lost its exact declaration owner",
            )]
        })
}

pub(crate) fn close_provider_declarations(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    plan: &effects::provider_plan::ProviderPlan,
    provider: &crate::ProviderPlanProvenance,
) -> Result<TerminalTargetProviderPlan, Vec<Diagnostic>> {
    let typed = context.typed;
    let nominal = |symbol| close_nominal_declaration(context, exact_toolchain_sources, symbol);
    if provider.row_realizations.len() != plan.rows.len()
        || provider.row_requirements.len() != plan.rows.len()
    {
        return Err(vec![Diagnostic::error(
            "selected provider lost row realization custody",
        )]);
    }
    let provider_declaration =
        close_selecting_declaration(context, exact_toolchain_sources, provider)?;
    let row_numeric_realizations = plan
        .rows
        .iter()
        .zip(&provider.row_realizations)
        .zip(&provider.row_requirements)
        .map(|((row, realization), requirement)| {
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) && typed.numeric_intrinsic_execution(*requirement).is_some()
            {
                realization.authored().map(nominal).transpose()
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(TerminalTargetProviderPlan {
        plan: plan.clone(),
        provider_declaration,
        row_numeric_realizations,
    })
}

pub fn retain_selected_provider_declarations(
    context: ProviderPlanningContext<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<Vec<TerminalTargetProviderPlan>, Vec<Diagnostic>> {
    if selected.plans().len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans have incomplete declaration correspondence",
        )]);
    }
    selected
        .plans()
        .iter()
        .zip(provenance)
        .map(|(plan, retained)| {
            if *plan != retained.plan {
                return Err(vec![Diagnostic::error(
                    "selected provider declarations belong to another plan",
                )]);
            }
            close_provider_declarations(context, exact_toolchain_sources, plan, &retained.provider)
        })
        .collect()
}
