//! Project the exact selected source associations for source-free binding.
//!
//! This is invocation scratch, not a second candidate registry. Derivation owns
//! named/captured row correspondence; Psi owns declaration evidence. Missing
//! custody is deferred until binding's original reach-admission phase.

use checked_trees::provider_declarations::{
    ProviderApplicationView, ProviderDeclarationRequest, ProviderDeclarationRow,
};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use provider_selection::SelectedPlanDeclarations;

pub fn prepare_selected_plan_declarations<'plans>(
    checked: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &'plans [crate::SelectedProviderReviewProvenance],
) -> Result<Vec<Result<SelectedPlanDeclarations<'plans>, Diagnostic>>, Diagnostic> {
    if selected.plans().len() != provenance.len() {
        return Err(Diagnostic::error(
            "installation reach lost selected provider custody",
        ));
    }
    let mut declarations = Vec::with_capacity(provenance.len());
    for (plan, retained) in selected.plans().iter().zip(provenance) {
        declarations.push(prepare_plan_declarations(checked, plan, retained));
    }
    Ok(declarations)
}

fn prepare_plan_declarations<'plans>(
    checked: &checked_trees::CheckedTrees,
    plan: &ProviderPlan,
    retained: &'plans crate::SelectedProviderReviewProvenance,
) -> Result<SelectedPlanDeclarations<'plans>, Diagnostic> {
    let provider = &retained.provider;
    if plan != &retained.plan
        || plan.rows.len() != provider.row_requirements.len()
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
            realization: realization.authored().unwrap_or_default(),
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
        installation_reach: !matches!(
            provider.schema,
            crate::ProviderSchemaDeclaration::BoundaryOperator(_)
                | crate::ProviderSchemaDeclaration::SealedAccessor(_)
        ),
    });
    Ok(SelectedPlanDeclarations {
        plan: &retained.plan,
        declarations,
    })
}
