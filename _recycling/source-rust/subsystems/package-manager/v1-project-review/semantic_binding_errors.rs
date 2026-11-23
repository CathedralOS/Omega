/*
    Parked from `source-rust/subsystems/package-manager/review/candidate/semantic_bindings.rs`
    on 2026-11-23. Its only callers are in this directory's `fresh_occurrences.rs`;
    restore it beside them when the fresh-occurrence review returns.

    `projection_error` turns a failed semantic-binding projection for one
    consumer into the review error the command reports: an ambiguous candidate
    binding, or the compilation diagnostics of missing callable facts.
*/

use build_evaluation_result::CompileFailure;
use review_evidence::SemanticBindingProjectionError;

use crate::declarations::PackageKey;
use crate::review::CompileResolvedPackageReviewsError;

pub(super) fn projection_error(
    consumer: &PackageKey,
    error: SemanticBindingProjectionError,
) -> CompileResolvedPackageReviewsError {
    match error {
        SemanticBindingProjectionError::Ambiguous {
            role,
            candidate_count,
        } => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::AmbiguousCandidateSemanticBinding {
                consumer: consumer.clone(),
                role,
                candidate_count,
            },
        ),
        SemanticBindingProjectionError::MissingCallableFacts { diagnostics } => {
            CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::Compilation {
                    package: consumer.clone(),
                    failure: CompileFailure::Diagnostics(diagnostics),
                },
            )
        }
    }
}
