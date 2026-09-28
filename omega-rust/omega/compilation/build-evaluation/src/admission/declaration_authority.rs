use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

pub(crate) fn admit_authored_declarations(
    typed: &typed_trees::TypedTrees,
    packages: &PackageCompilationInputs,
    generated_source_custody: &[(source::SourceId, build_output::PackageGeneratedSource)],
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<package_compilation::PackageSourceConsumptionCommitment, Vec<Diagnostic>> {
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
    timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(
                provisional,
                &typed_trees_to_checked_trees::CheckingRequest::preliminary()
                    .with_declaration_authority(Some(packages)),
            );
            artifacts::compile_timings::counters::record_preliminary_check(started.elapsed());
            result.map(|_| ())
        },
    )?;
    // Source identity comes from the frontend snapshot, not execution facts.
    // No checked product is published here, so there is no product to prune.
    // The authority check above remains until its selection obligations can
    // be admitted without repeating whole-program checking.
    let base = package_compilation::derive_package_compilation_subject(
        typed.symbols.source_files(),
        packages,
        generated_source_custody,
    )?;
    Ok(base.source_consumption_commitment())
}
