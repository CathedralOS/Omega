use super::{CompileResolvedPackageReviewsError, PackageSourceVerificationPhase};
use crate::declarations::PackageKey;
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::timings;
use package_source::local::operations::verify_package_source_snapshot;

pub(crate) fn verify_transitive_source_custody(
    closure: &ResolvedPackageSourceClosure,
    compiling_package: &PackageKey,
    phase: PackageSourceVerificationPhase,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let _stage = timings::stage("source_custody_verification");
    verify_selected_source_custody(
        &PackageCompilationScope::new(closure, compiling_package),
        phase,
    )
}

pub(super) fn verify_selected_source_custody(
    scope: &PackageCompilationScope<'_>,
    phase: PackageSourceVerificationPhase,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let closure = scope.closure();
    let compiling_package = scope.root();
    for source_package in scope.packages() {
        let custody = closure
            .custody(source_package)
            .expect("validated source closure retains every reachable custody");
        // A retained capture owns the exact bytes consumed by compilation; its
        // routing path need not be a sealed on-disk cache directory. Persistent
        // snapshots still require physical verification. Publication separately
        // compares live local sources with the reviewed immutable content.
        if custody.captured_source().is_none() {
            verify_package_source_snapshot(
                custody.snapshot_root(),
                custody.materialization().content(),
                custody.source_limits(),
            )
            .map_err(|error| CompileResolvedPackageReviewsError::SourceCustody {
                compiling_package: compiling_package.clone(),
                source_package: source_package.clone(),
                phase,
                error,
            })?;
        }
        custody.selection_evidence().revalidate().map_err(|error| {
            CompileResolvedPackageReviewsError::SourceSelectionCustody {
                compiling_package: compiling_package.clone(),
                source_package: source_package.clone(),
                phase,
                error,
            }
        })?;
    }
    Ok(())
}
