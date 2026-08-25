use crate::pipeline::PackageCompilationInputs;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use psi_source::SourceOrigin;
use psi_symbols::SymbolKind;

pub(super) fn validate_authored_declaration_selections_before_build(
    typed: &psi_typed_trees::TypedTrees,
    packages: &PackageCompilationInputs,
    timings: &mut crate::pipeline::timing::CompileTimings,
) -> Result<(), Vec<Diagnostic>> {
    // Package build execution can carry filesystem and other boundary
    // authority. Check the frozen ordinary source graph first; the ordinary
    // final checked pass repeats this gate after any explicit generated-source
    // handoff.
    let checked = crate::pipeline::stages::typed_trees_to_checked_trees(typed.clone(), timings)?;
    validate_authored_declaration_selections(&checked.program, packages)
}

pub(super) fn validate_authored_declaration_selections(
    program: &CheckedTrees,
    packages: &PackageCompilationInputs,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for selection in program.authored_declaration_selections() {
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
                        "package-authored declaration selection remained unresolved after successful checking: {:?} ({binding:?})",
                        selection.kind(),
                    ))
                    .with_source_span(source_span),
                );
                continue;
            }
            AuthoredDeclarationSelectionTarget::Resolved(selected) => selected.selected_symbol(),
        };

        // Primitive types and compiler builtin functions have exact semantic
        // identity but intentionally have no package or authored toolchain
        // source owner. They cannot be spoofed by a package declaration: the
        // resolved symbol kind, not spelling, selects this lane.
        if matches!(
            program.symbols.get(selected).kind,
            SymbolKind::BuiltinType | SymbolKind::BuiltinFunction
        ) {
            continue;
        }

        if program.symbols.get(selected).kind == SymbolKind::Proposition {
            let Some(declaration) = program
                .propositions()
                .iter()
                .find(|declaration| declaration.symbol == selected)
            else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "selected proposition `{}` has no retained declaration visibility",
                        program.symbols.display_path(selected, "::")
                    ))
                    .with_source_span(source_span),
                );
                continue;
            };
            if !declaration.is_public {
                let owner = program.symbols.symbol_package_identity(selected);
                if owner != Some(requester) {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "package {} selects private proposition `{}`",
                            packages.package_label(requester),
                            program.symbols.display_path(selected, "::"),
                        ))
                        .with_source_span(source_span),
                    );
                    continue;
                }
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

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
