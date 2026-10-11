//! Package completion roles enter source checking before provider selection.
//! Confined review may nominate a portable declaration, but it must join that
//! same declaration to a complete provider proposal before review can continue.
//! Ordinary compilation admits only consumer-accepted roles. Neither route
//! rewrites checked source to match a selected provider.

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
        if inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery) {
            nominate_source_completion(typed)?;
        }
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

/// Review requested source-role discovery explicitly. Names nominate an inert
/// review subject, never an accepted binding. The exact source declaration is
/// fixed before checking and before any target chooses a provider. Package
/// review subsequently recompiles with the full proposed schema/plan binding;
/// execution still requires consumer policy acceptance.
fn nominate_source_completion(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let declarations = typed
        .traits()
        .iter()
        .filter_map(|definition| {
            if !crate::service_schema::is_product_declaration(typed, definition.symbol)
                || typed.symbols.display_path(definition.symbol, "::") != "Console"
            {
                return None;
            }
            let package = typed.symbols.symbol_package_identity(definition.symbol)?;
            let declaration = completion_declaration(typed, definition.symbol, package)?;
            has_external_source_candidate(typed, &declaration).then_some(declaration)
        })
        .collect::<Vec<_>>();
    if declarations.len() > 1 {
        return Err(vec![Diagnostic::error(
            "completion role discovery has multiple package-owned source declarations",
        )]);
    }
    for declaration in declarations {
        let _ = typed.boundary_completion_declarations.append(declaration);
    }
    Ok(())
}

/// A same-named service implemented entirely by ordinary returning bodies is
/// not an exit-role review subject. Nomination considers the complete authored
/// external-supply roster, never a target-selected implementation or ABI. This
/// only selects a provisional review subject; settlement must still join an
/// actual external realization and ordinary compilation requires acceptance.
fn has_external_source_candidate(
    typed: &TypedTrees,
    declaration: &BoundaryCompletionDeclaration,
) -> bool {
    typed.machines().iter().any(|machine| {
        !machine.body_is_present
            && matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::Boundary
                    | language_semantics::MachineSupplyMode::ExternalRealization { .. }
            )
            && crate::service_schema::is_product_declaration(typed, machine.symbol)
            && typed.symbols.symbol_package_identity(machine.symbol) == declaration.package
            && typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    checked_trees::typed_trees::machine::resolve_satisfied_declaration(
                        typed,
                        machine,
                        conformance,
                    )
                    .is_some_and(|requirement| requirement.symbol() == declaration.requirement)
                })
    })
}

/// Rejoin every provisional source role to its exact selected realization.
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
