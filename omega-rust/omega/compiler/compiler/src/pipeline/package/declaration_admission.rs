use crate::pipeline::PackageCompilationInputs;
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceOrigin;
use symbols::SymbolKind;

/// Opaque proof that the exact pre-build package source closure passed the
/// authored-declaration authority gate.
///
/// The retained commitment belongs to the base frontend before this
/// activation appends any own generated source. Construction stays private so
/// build orchestration cannot pair an authority verdict with a different
/// source closure.
pub(super) struct AuthoredDeclarationAuthorityVerdict {
    base_source_consumption_commitment: package_compilation::PackageSourceConsumptionCommitment,
}

impl AuthoredDeclarationAuthorityVerdict {
    pub(super) const fn base_source_consumption_commitment(
        &self,
    ) -> package_compilation::PackageSourceConsumptionCommitment {
        self.base_source_consumption_commitment
    }
}

pub(super) fn validate_authored_declaration_selections_before_build(
    typed: &typed_trees::TypedTrees,
    packages: &PackageCompilationInputs,
    generated_source_custody: &[(source::SourceId, build_output::PackageGeneratedSource)],
    timings: &mut crate::pipeline::timing::CompileTimings,
) -> Result<AuthoredDeclarationAuthorityVerdict, Vec<Diagnostic>> {
    // Package build execution can carry filesystem and other boundary
    // authority. Check the frozen ordinary source graph first; the ordinary
    // final checked pass repeats this gate after any explicit generated-source
    // handoff.
    let checked = crate::pipeline::phase_transitions::typed_trees_to_preliminary_checked_trees(
        typed.clone(),
        timings,
    )?;
    validate_authored_declaration_selections(&checked, packages)?;
    let base = package_compilation::derive_package_compilation_subject(
        &checked,
        packages,
        generated_source_custody,
    )?;
    Ok(AuthoredDeclarationAuthorityVerdict {
        base_source_consumption_commitment: base.source_consumption_commitment(),
    })
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

        let selected_kind = program.symbols.get(selected).kind;
        if typed_trees::visibility::requires_declaration_visibility(selected_kind) {
            let Some(visibility) =
                typed_trees::visibility::declaration_visibility(program, selected)
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

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
