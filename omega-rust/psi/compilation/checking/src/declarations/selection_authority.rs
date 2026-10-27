//! Package authority for authored declaration selections.
//!
//! The caller supplies reconciled dependency authority and the exact selection
//! occurrences excluded by its target-sibling checkpoint. Psi rejoins source
//! custody, resolved declaration identity, visibility and direct dependencies.
//! Loading a transitive package does not make its declarations selectable.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceOrigin;
use std::collections::HashSet;

/// Package-neutral authority consulted before the compiler executes an
/// authored machine or admits checked declaration selections.
///
/// Psi owns provenance and call-closure discovery. Omega supplies the exact
/// reconciled direct-dependency predicate without leaking resolver or lockfile
/// structures into the language layer.
pub trait DeclarationSelectionAuthority: Send + Sync {
    fn allows_declaration_selection(
        &self,
        requester: semantic_vocabulary::PackageKeyIdentity,
        owner: semantic_vocabulary::PackageKeyIdentity,
    ) -> bool;

    fn package_label(&self, identity: semantic_vocabulary::PackageKeyIdentity) -> String;
}

/// Validate exact checked declaration selections under supplied package authority.
/// The caller derives `skipped` from target-sibling bodies excluded by its
/// checkpoint, whether pruned provisionally or retained through settled checking.
/// This checks the supplied scope; it does not authorize that scope or derive
/// package admission from loaded source.
pub fn validate_authored_declaration_selections(
    program: &CheckedTrees,
    packages: &dyn DeclarationSelectionAuthority,
    skipped: &HashSet<typed_trees::AuthoredDeclarationSelectionOccurrenceId>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for selection in program.facts.authored_declaration_selections() {
        if skipped.contains(&selection.occurrence_id()) {
            continue;
        }
        let source_span = selection.source_span();
        let Some(source_file) = program.symbols.source_file(source_span) else {
            diagnostics.push(
                Diagnostic::error(
                    "authored declaration selection has no compiler-owned source custody",
                )
                .with_source_span(source_span),
            );
            continue;
        };
        // Toolchain source is not a managed package requester. Its own
        // selections remain compiler-TCB input and must not be projected as
        // dependency authority of the package being compiled.
        if source_file.origin == SourceOrigin::Toolchain {
            continue;
        }
        let Some(requester) = source_file.package_identity else {
            diagnostics.push(
                Diagnostic::error(format!(
                    "authored declaration selection in {} has no reconciled requesting package identity",
                    source_file.path.display()
                ))
                .with_source_span(source_span),
            );
            continue;
        };

        let selected = match selection.target() {
            AuthoredDeclarationSelectionTarget::Intrinsic(_) => continue,
            AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "package-authored declaration selection `{}` remained unresolved after successful checking: {:?} ({binding:?})",
                        program.symbols.source_text(source_span),
                        selection.kind(),
                    ))
                    .with_source_span(source_span),
                );
                continue;
            }
            AuthoredDeclarationSelectionTarget::Resolved(selected) => selected.selected_symbol(),
        };
        // A selection recorded inside a target sibling's body was checked for
        // that target and pruned with it; it is not a selection this
        // realization admits.
        let declarations = program.product_declarations();
        if !typed_trees::visibility::owning_machine_is_retained(declarations, selected) {
            continue;
        }

        // Installed primitive and mathematical declarations have exact
        // compiler identity but intentionally no authored source/package owner.
        // Slot queries retain the installed handle, including a type member's
        // fixed parent; a user declaration with the same spelling gains no
        // authority. The fixed core vocabulary is mathematical, so its symbol
        // kind alone cannot distinguish it from an authored definition.
        if program.symbols.builtin_type_atom(selected).is_some()
            || program
                .symbols
                .builtin_function_for_symbol(selected)
                .is_some()
            || program.symbols.builtin_type_member(selected).is_some()
            || symbols::FixedCoreDeclaration::ALL
                .into_iter()
                .any(|declaration| {
                    program.symbols.fixed_core_declaration_symbol(declaration) == Some(selected)
                })
        {
            continue;
        }

        let selected_kind = program.symbols.get(selected).kind;
        if typed_trees::visibility::requires_declaration_visibility(selected_kind) {
            let Some(visibility) =
                typed_trees::visibility::authored_selection_visibility(declarations, selected)
            else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "selected {:?} `{}` has no retained declaration visibility",
                        selected_kind,
                        program.symbols.display_path(selected, "::")
                    ))
                    .with_source_span(source_span),
                );
                continue;
            };
            let owner = program.symbols.symbol_package_identity(selected);
            if !visibility.is_public() && owner != Some(requester) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "package {} selects private {} `{}`",
                        packages.package_label(requester),
                        visibility.kind(),
                        program.symbols.display_path(selected, "::"),
                    ))
                    .with_source_span(source_span),
                );
                continue;
            }
        }

        if let Some(owner) = program.symbols.symbol_package_identity(selected) {
            if !packages.allows_declaration_selection(requester, owner) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "package {} selects declaration `{}` owned by package {} without declaring that package as a direct dependency",
                        packages.package_label(requester),
                        program.symbols.display_path(selected, "::"),
                        packages.package_label(owner),
                    ))
                    .with_source_span(source_span),
                );
            }
            continue;
        }

        match program.symbols.symbol_source_origin(selected) {
            Some(SourceOrigin::Toolchain) => {}
            Some(SourceOrigin::User) => diagnostics.push(
                Diagnostic::error(format!(
                    "selected user declaration `{}` has no reconciled owning package identity",
                    program.symbols.display_path(selected, "::")
                ))
                .with_source_span(source_span),
            ),
            None => diagnostics.push(
                Diagnostic::error(format!(
                    "selected declaration `{}` has no package or toolchain provenance",
                    program.symbols.display_path(selected, "::")
                ))
                .with_source_span(source_span),
            ),
        }
    }

    crate::program_validation::finish_strict_diagnostics(diagnostics)
}
