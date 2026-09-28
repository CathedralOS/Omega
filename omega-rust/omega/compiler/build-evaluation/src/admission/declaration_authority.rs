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
    let checked = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(
                provisional,
                &typed_trees_to_checked_trees::CheckingRequest::preliminary()
                    .with_declaration_authority(Some(packages)),
            );
            record_preliminary_check(started.elapsed());
            typed_trees_to_checked_trees::prune_target_siblings(result?)
        },
    )?;
    let base = package_compilation::derive_package_compilation_subject(
        &checked,
        packages,
        generated_source_custody,
    )?;
    Ok(base.source_consumption_commitment())
}

static PRELIMINARY_CHECK_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PRELIMINARY_CHECK_MICROS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Monotonic `(calls, wall microseconds)` totals of the pre-build
/// declaration-authority checks `lower_typed_trees` served. A call served
/// from the exact-input check memo returns in microseconds while a fresh
/// check of a package graph takes seconds, so a review pass whose
/// preliminary total stays near zero while its call count rises was already
/// deduplicated, and the microseconds that remain name the residual
/// preliminary-versus-settled duplication inside each compile.
#[doc(hidden)]
pub fn preliminary_check_totals() -> (u64, u64) {
    (
        PRELIMINARY_CHECK_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        PRELIMINARY_CHECK_MICROS.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Account one pre-build declaration-authority check's wall time.
fn record_preliminary_check(elapsed: std::time::Duration) {
    PRELIMINARY_CHECK_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    PRELIMINARY_CHECK_MICROS.fetch_add(
        u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
        std::sync::atomic::Ordering::Relaxed,
    );
}
