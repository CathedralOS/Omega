use crate::package_compilation::PackageCompilationInputs;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use source::SourceOrigin;
use std::collections::HashSet;
use symbols::SymbolKind;
use typed_trees_to_checked_trees::checked_trees::CheckedTrees;

/// Opaque proof that the exact pre-build package source closure passed the
/// authored-declaration authority gate.
///
/// The retained commitment belongs to the base frontend before this
/// activation appends any own generated source. Construction stays private so
/// build orchestration cannot pair an authority verdict with a different
/// source closure.
pub(crate) struct AuthoredDeclarationAuthorityVerdict {
    base_source_consumption_commitment:
        crate::package_compilation::PackageSourceConsumptionCommitment,
}

impl AuthoredDeclarationAuthorityVerdict {
    pub(crate) const fn base_source_consumption_commitment(
        &self,
    ) -> crate::package_compilation::PackageSourceConsumptionCommitment {
        self.base_source_consumption_commitment
    }
}

pub(crate) fn validate_authored_declaration_selections_before_build(
    typed: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    packages: &PackageCompilationInputs,
    generated_source_custody: &[(
        source::SourceId,
        crate::build_output::PackageGeneratedSource,
    )],
    timings: &mut crate::artifacts::compile_timings::CompileTimings,
) -> Result<AuthoredDeclarationAuthorityVerdict, Vec<Diagnostic>> {
    // Package build execution can carry filesystem and other boundary
    // authority. Check the frozen ordinary source graph first; the ordinary
    // final checked pass repeats this gate after any explicit generated-source
    // handoff.
    // The other targets' sibling bodies are checked once, in the settled
    // pass; this provisional pass admits declarations only.
    let mut provisional = typed.clone();
    let pruned_occurrences =
        typed_trees_to_checked_trees::prune_target_siblings_typed(&mut provisional);
    let checked =
        crate::compiler::checked::checking::phase_transitions::typed_trees_to_preliminary_checked_trees(
            provisional,
            timings,
        )?;
    validate_authored_declaration_selections_skipping(&checked, packages, &pruned_occurrences)?;
    let base = crate::package_compilation::derive_package_compilation_subject(
        &checked,
        packages,
        generated_source_custody,
    )?;
    Ok(AuthoredDeclarationAuthorityVerdict {
        base_source_consumption_commitment: base.source_consumption_commitment(),
    })
}

pub(crate) fn validate_authored_declaration_selections(
    program: &CheckedTrees,
    packages: &PackageCompilationInputs,
) -> Result<(), Vec<Diagnostic>> {
    // Siblings are still present at this point; their selections resolve
    // under their own target's realization and are pruned with them.
    let sibling_occurrences =
        typed_trees_to_checked_trees::target_sibling_selection_occurrences(program);
    validate_authored_declaration_selections_skipping(program, packages, &sibling_occurrences)
}

/// `skipped` names the selections recorded inside bodies a provisional check
/// pruned before checking; they belong to another target's realization.
fn validate_authored_declaration_selections_skipping(
    program: &CheckedTrees,
    packages: &PackageCompilationInputs,
    skipped: &HashSet<
        symbol_resolved_trees_to_typed_trees::typed_trees::AuthoredDeclarationSelectionOccurrenceId,
    >,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for selection in program.authored_declaration_selections() {
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
        if !symbol_resolved_trees_to_typed_trees::typed_trees::visibility::owning_machine_is_retained(program, selected) {
            continue;
        }

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
        if symbol_resolved_trees_to_typed_trees::typed_trees::visibility::requires_declaration_visibility(selected_kind) {
            let Some(visibility) =
                symbol_resolved_trees_to_typed_trees::typed_trees::visibility::declaration_visibility(program, selected)
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
