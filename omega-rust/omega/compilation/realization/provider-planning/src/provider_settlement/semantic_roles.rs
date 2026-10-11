//! Accepted package roles enter source checking before provider selection.
//! Discovery cannot rewrite a completed source judgment to match a provider.

use crate::{ProviderSchemaDeclaration, SelectedProviderPlanWithProvenance};
use checked_trees::typed_trees::{BoundaryCompletionDeclaration, TypedTrees};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use package_compilation::{AcceptedSemanticBindingRole, PackageCompilationInputs};
use semantic_vocabulary::{BoundaryCompletion, ExternalCompletionKind, PackageKeyIdentity};
use symbols::SymbolHandle;

/// Resolve an accepted package role to its exact source requirement. This
/// precedes checking and never observes a selected realization provider.
pub fn prepare_source_completion(
    typed: &mut TypedTrees,
    inputs: Option<&PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    // Source checkpoints are reusable across policy requests, so every request
    // reconstructs its own role inputs rather than inheriting a prior binding.
    typed.boundary_completion_declarations.clear();
    let Some(binding) = inputs.and_then(|inputs| {
        inputs.accepted_semantic_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32)
    }) else {
        return Ok(());
    };
    let declarations = typed
        .traits()
        .iter()
        .filter(|definition| {
            crate::service_schema::is_product_declaration(typed, definition.symbol)
                && typed.symbols.symbol_package_identity(definition.symbol)
                    == Some(binding.package())
                && typed.symbols.display_path(definition.symbol, "::") == binding.declaration_path()
        })
        .collect::<Vec<_>>();
    let [definition] = declarations.as_slice() else {
        return Err(vec![Diagnostic::error(
            "accepted completion role has no unique package-owned declaration",
        )]);
    };
    let declaration = completion_declaration(typed, definition.symbol, binding.package())
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "accepted completion role has no exact process-exit requirement",
            )]
        })?;
    let _ = typed.boundary_completion_declarations.append(declaration);
    Ok(())
}

/// Discovery cannot change the source judgment to fit a selected provider.
/// A new completion meaning must enter through source admission before checking.
pub(super) fn validate_proposed(
    typed: &TypedTrees,
    selected: &[SelectedProviderPlanWithProvenance],
) -> Result<(), Vec<Diagnostic>> {
    if !typed.boundary_completion_declarations.is_empty() {
        return Ok(());
    }
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
        let has_external_row = plan.rows.iter().enumerate().any(|(index, row)| {
            matches!(
                row.binding,
                ProviderBinding::CompilerIntrinsic { .. } | ProviderBinding::Import { .. }
            ) && provenance.row_requirements.get(index) == Some(&declaration.requirement)
                && provenance
                    .row_realizations
                    .get(index)
                    .is_some_and(|realization| {
                        typed.symbols.symbol_package_identity(*realization) == Some(package)
                    })
        });
        if has_external_row {
            declarations.push(declaration);
        }
    }
    if declarations.len() > 1 {
        return Err(vec![Diagnostic::error(
            "completion role discovery has multiple selected package-owned Console declarations",
        )]);
    }
    if !declarations.is_empty() {
        return Err(vec![Diagnostic::error(
            "unimplemented: portable completion-role discovery; selected providers cannot change a checked source contract",
        )]);
    }
    Ok(())
}

fn completion_declaration(
    typed: &TypedTrees,
    owner: SymbolHandle,
    package: PackageKeyIdentity,
) -> Option<BoundaryCompletionDeclaration> {
    let definition = typed.trait_definition_by_symbol(owner)?;
    let mut requirements = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == "exit_process");
    let requirement = requirements.next()?;
    if requirements.next().is_some() {
        return None;
    }
    let declaration = BoundaryCompletionDeclaration {
        package: Some(package),
        owner,
        owner_path: typed.symbols.display_path(owner, "::"),
        requirement: requirement.symbol,
        requirement_path: typed.symbols.display_path(requirement.symbol, "::"),
        completion: BoundaryCompletion::External(ExternalCompletionKind::ProcessExit),
    };
    checked_trees::typed_trees::boundary_completions::valid_boundary_completion_declaration(
        typed,
        &declaration,
    )
    .then_some(declaration)
}
