//! Rejoin source completion declarations to selected provider realizations.
//! Source admission belongs to Psi; this operation never mutates its judgment.

use crate::{ProviderSchemaDeclaration, SelectedProviderPlanWithProvenance};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::boundary_completions::completion_declaration;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;

/// Rejoin selected provisional source roles to their exact realizations.
/// This validates the proposal without changing any completed source judgment.
pub(super) fn validate_proposed(
    typed: &TypedTrees,
    selected: &[SelectedProviderPlanWithProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let mut declarations = Vec::new();
    for selected in selected {
        let plan = &selected.derived.plan;
        let provenance = &selected.derived.provenance;
        let ProviderSchemaDeclaration::BoundaryTrait(owner) = provenance.schema else {
            continue;
        };
        let Some(package) = plan.provider_type_package_identity else {
            continue;
        };
        if plan.schema.identity_text(plan.schema.trait_name) != "Console"
            || plan.schema.trait_package_identity != Some(package)
            || plan.origin_package_identity != Some(package)
            || typed.symbols.symbol_package_identity(owner) != Some(package)
        {
            continue;
        }
        let Some(declaration) = completion_declaration(typed, owner, package) else {
            continue;
        };
        let external_rows = plan
            .rows
            .iter()
            .enumerate()
            .filter(|(index, row)| {
                matches!(
                    row.binding,
                    ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Import { .. }
                ) && provenance.row_requirements.get(*index) == Some(&declaration.requirement)
                    && provenance
                        .row_realizations
                        .get(*index)
                        .is_some_and(|realization| {
                            typed.symbols.symbol_package_identity(*realization) == Some(package)
                        })
            })
            .count();
        if external_rows > 1 {
            return Err(vec![Diagnostic::error(
                "completion role discovery has multiple realizations for one source requirement",
            )]);
        }
        if external_rows == 1 {
            declarations.push(declaration);
        }
    }
    if declarations.len() > 1 {
        return Err(vec![Diagnostic::error(
            "completion role discovery has multiple selected package-owned Console declarations",
        )]);
    }
    let proposals = typed
        .boundary_completion_declarations
        .iter()
        .map(|(_, declaration)| declaration)
        // Source nomination covers every target body. A library can declare
        // hosted exit providers while being reviewed for a freestanding target
        // that selects none of them. Only a selected schema requires this
        // realization join; source checking stays target-neutral, and ordinary
        // provider-demand checking still rejects an actual unsupported call.
        .filter(|declaration| {
            selected.iter().any(|provider| {
                matches!(
                    provider.derived.provenance.schema,
                    ProviderSchemaDeclaration::BoundaryTrait(owner)
                        if owner == declaration.owner
                )
            })
        })
        .collect::<Vec<_>>();
    if proposals.len() != declarations.len()
        || proposals.iter().any(|proposal| {
            !declarations.iter().any(|declaration| {
                proposal.package == declaration.package
                    && proposal.owner == declaration.owner
                    && proposal.requirement == declaration.requirement
                    && proposal.completion == declaration.completion
            })
        })
    {
        return Err(vec![Diagnostic::error(
            "completion role discovery does not match the exact source declaration and selected realization",
        )]);
    }
    Ok(())
}
