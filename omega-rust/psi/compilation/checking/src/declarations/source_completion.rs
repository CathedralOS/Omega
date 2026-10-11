//! Source completion roles are established before checking, never by provider selection.
//!
//! Package policy supplies a declaration identity or requests confined nomination.
//! Neither request installs a provider or authorizes execution. Omega rejoins the
//! resulting declarations to selected realizations without changing this judgment.

use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::boundary_completions::completion_declaration;
use typed_trees::{BoundaryCompletionDeclaration, TypedTrees};

/// Fresh invocation policy for package-owned completion declarations.
/// The toolchain's canonical declarations remain independently recognized.
#[derive(Clone, Copy, Default)]
pub enum SourceCompletionAdmission<'request> {
    #[default]
    None,
    AcceptedProcessExit {
        package: PackageKeyIdentity,
        declaration_path: &'request str,
    },
    /// Nominate from the complete source roster for confined package review.
    /// This supplies no consumer acceptance or runtime authority.
    DiscoverProcessExit,
}

/// Resolve an accepted package role to its exact source requirement. This
/// precedes checking and never observes a selected realization provider.
pub fn prepare_source_completion(
    typed: &mut TypedTrees,
    admission: SourceCompletionAdmission<'_>,
) -> Result<(), Vec<Diagnostic>> {
    // Source checkpoints are reusable across policy requests, so every request
    // reconstructs its own role inputs rather than inheriting a prior binding.
    typed.boundary_completion_declarations.clear();
    let SourceCompletionAdmission::AcceptedProcessExit {
        package,
        declaration_path,
    } = admission
    else {
        if matches!(admission, SourceCompletionAdmission::DiscoverProcessExit) {
            nominate_source_completion(typed)?;
        }
        return Ok(());
    };
    let declarations = typed
        .traits()
        .iter()
        .filter(|definition| {
            is_product_declaration(typed, definition.symbol)
                && typed.symbols.symbol_package_identity(definition.symbol) == Some(package)
                && typed.symbols.display_path(definition.symbol, "::") == declaration_path
        })
        .collect::<Vec<_>>();
    let [definition] = declarations.as_slice() else {
        return Err(vec![Diagnostic::error(
            "accepted completion role has no unique package-owned declaration",
        )]);
    };
    let declaration =
        completion_declaration(typed, definition.symbol, package).ok_or_else(|| {
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
            if !is_product_declaration(typed, definition.symbol)
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
            && is_product_declaration(typed, machine.symbol)
            && typed.symbols.symbol_package_identity(machine.symbol) == declaration.package
            && typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)
                        .is_some_and(|requirement| requirement.symbol() == declaration.requirement)
                })
    })
}

fn is_product_declaration(typed: &TypedTrees, symbol: SymbolHandle) -> bool {
    typed
        .symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| typed.symbols.source_file(span))
        .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
}
