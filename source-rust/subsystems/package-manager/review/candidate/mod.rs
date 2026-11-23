//! Review the packages in one resolved source closure and retain the results for later passes of this invocation.

mod custody;
mod error;
mod passes;
mod review_set;
mod semantic_bindings;

pub(crate) use custody::verify_transitive_source_custody;
pub use error::{CompileResolvedPackageReviewCause, CompileResolvedPackageReviewsError};
pub(crate) use passes::{CandidateReview, PackageSourceBindings, review_resolved_packages};
pub use review_set::PackageSourceVerificationPhase;
pub(crate) use review_set::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
pub(crate) use semantic_bindings::ConsumerScopedSemanticBindingReviewInput;

pub(crate) fn build_observation_commitment(
    summary: &package_compilation::build_execution::BuildObservationSummary,
) -> [u8; 32] {
    summary.identity().digest()
}
