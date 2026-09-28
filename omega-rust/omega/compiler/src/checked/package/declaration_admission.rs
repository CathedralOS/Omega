use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

/// Opaque proof that the exact pre-build package source closure passed the
/// authored-declaration authority gate.
///
/// The retained commitment belongs to the base frontend before this
/// activation appends any own generated source. Construction stays private so
/// build orchestration cannot pair an authority verdict with a different
/// source closure.
pub(crate) struct AuthoredDeclarationAuthorityVerdict {
    base_source_consumption_commitment: package_compilation::PackageSourceConsumptionCommitment,
}

impl AuthoredDeclarationAuthorityVerdict {
    pub(crate) const fn base_source_consumption_commitment(
        &self,
    ) -> package_compilation::PackageSourceConsumptionCommitment {
        self.base_source_consumption_commitment
    }
}

pub(crate) fn validate_authored_declaration_selections_before_build(
    typed: &typed_trees::TypedTrees,
    packages: &PackageCompilationInputs,
    generated_source_custody: &[(source::SourceId, build_output::PackageGeneratedSource)],
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<AuthoredDeclarationAuthorityVerdict, Vec<Diagnostic>> {
    // Package build execution can carry filesystem and other boundary
    // authority. Check the frozen ordinary source graph first; the ordinary
    // final checked pass repeats this gate after any explicit generated-source
    // handoff.
    // The other targets' sibling bodies are checked once, in the settled
    // pass; this provisional pass admits declarations only.
    // Its result deliberately cannot feed that settled check: the settled
    // program differs (the build may append generated source, target
    // selection swaps canonical bodies, provider settlement records erasure
    // authorizations, and const folds land the endpoints only the
    // preliminary mode tolerates pending), and the settled request closes
    // those tolerances and carries the specialization rosters this request
    // leaves empty. Only the exact-input check memo deduplicates it — for a
    // later pass that arrives at this same frozen graph under this same
    // request.
    let mut provisional = typed.clone();
    typed_trees_to_checked_trees::prune_target_siblings_typed(&mut provisional);
    let checked = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(
                provisional,
                &typed_trees_to_checked_trees::CheckingRequest::preliminary()
                    .with_declaration_authority(Some(packages)),
            );
            crate::checked::checking::record_preliminary_check(started.elapsed());
            typed_trees_to_checked_trees::prune_target_siblings(result?)
        },
    )?;
    let base = package_compilation::derive_package_compilation_subject(
        &checked,
        packages,
        generated_source_custody,
    )?;
    Ok(AuthoredDeclarationAuthorityVerdict {
        base_source_consumption_commitment: base.source_consumption_commitment(),
    })
}
