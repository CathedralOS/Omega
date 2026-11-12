// We capture and attach a selected provider implementation's call and effect
// declarations before Terminal publication. Selection keeps each captured
// `ProviderDeclarations` result with its original candidate so ABI argument
// placement can change without reopening source; errors remain deferred until
// binding checks the selected calls.

use checked_trees::provider_declarations::{
    ProviderApplicationView, ProviderDeclarationOrigin, ProviderDeclarationRequest,
    ProviderDeclarationRow,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use provider_selection::SelectedPlanDeclarations;

pub(crate) fn capture_plan_declarations(
    checked: &checked_trees::CheckedTrees,
    plan: &ProviderPlan,
    provider: &crate::ProviderPlanProvenance,
) -> Result<language_semantics::provider_declarations::ProviderDeclarations, Diagnostic> {
    if plan.rows.len() != provider.row_requirements.len()
        || plan.rows.len() != provider.row_realizations.len()
    {
        return Err(Diagnostic::error(
            "installation reach lost exact selected row custody",
        ));
    }
    let mut rows = Vec::with_capacity(plan.rows.len());
    for ((row, requirement), realization) in plan
        .rows
        .iter()
        .zip(&provider.row_requirements)
        .zip(&provider.row_realizations)
    {
        rows.push(ProviderDeclarationRow {
            requirement: *requirement,
            origin: match realization {
                crate::ProviderRowRealization::Authored(symbol) => {
                    ProviderDeclarationOrigin::Authored(*symbol)
                }
                crate::ProviderRowRealization::BuildNative(native) => {
                    ProviderDeclarationOrigin::BuildNative {
                        requirement: native.requirement,
                    }
                }
                crate::ProviderRowRealization::Toolchain => ProviderDeclarationOrigin::Toolchain,
            },
            requirement_identity: plan.identities.text(row.requirement_identity),
            application: match row.binding {
                ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Syscall { .. } => {
                    ProviderApplicationView::PhysicalLeaf
                }
                ProviderBinding::Import { .. } => ProviderApplicationView::External,
                _ => ProviderApplicationView::None,
            },
        });
    }
    let declarations = checked.provider_declarations(ProviderDeclarationRequest {
        rows: &rows,
        named_application: provider.named_application,
        native_application: (provider.named_application.is_none()
            && provider
                .row_realizations
                .iter()
                .any(|row| matches!(row, crate::ProviderRowRealization::BuildNative(_))))
        .then(|| plan.application_identity())
        .flatten(),
        installation_reach: !matches!(
            provider.schema,
            crate::ProviderSchemaDeclaration::BoundaryOperator(_)
                | crate::ProviderSchemaDeclaration::SealedAccessor(_)
        ),
    });
    Ok(declarations)
}

pub fn prepare_selected_plan_declarations<'plans>(
    captured: Vec<
        Result<language_semantics::provider_declarations::ProviderDeclarations, Diagnostic>,
    >,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &'plans [crate::SelectedProviderReviewProvenance],
) -> Result<Vec<Result<SelectedPlanDeclarations<'plans>, Diagnostic>>, Diagnostic> {
    if selected.plans().len() != provenance.len() || captured.len() != provenance.len() {
        return Err(Diagnostic::error(
            "installation reach lost selected provider custody",
        ));
    }
    Ok(selected
        .plans()
        .iter()
        .zip(provenance)
        .zip(captured)
        .map(|((plan, retained), declarations)| {
            if plan != &retained.plan
                || plan.rows.len() != retained.provider.row_requirements.len()
                || plan.rows.len() != retained.provider.row_realizations.len()
            {
                return Err(Diagnostic::error(
                    "installation reach lost exact selected row custody",
                ));
            }
            Ok(SelectedPlanDeclarations {
                plan: &retained.plan,
                declarations: declarations?,
            })
        })
        .collect())
}
