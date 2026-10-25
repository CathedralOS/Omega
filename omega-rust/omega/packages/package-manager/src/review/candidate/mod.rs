//! Admit review work and join its evidence to one resolved immutable closure.

mod commitments;
mod custody;
mod error;
mod passes;
mod review_cache;
mod review_set;
mod semantic_bindings;

pub(crate) use commitments::build_observation_commitment;
pub(crate) use custody::verify_transitive_source_custody;
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use passes::{CandidateReview, PackageSourceBindings, review_resolved_packages};
pub use passes::{
    PackageReviewCompletion, PackageReviewComponent, PackageReviewExecutor,
    PackageReviewOccurrence, PackageReviewPass, PackageReviewRound, PackageReviewWork,
    RetainedPackageReviews,
};
pub use review_cache::PackageReviewCacheWrite;
pub(crate) use review_set::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
pub(crate) use semantic_bindings::ConsumerScopedSemanticBindingReviewInput;
