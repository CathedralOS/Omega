//! Compile one resolved immutable closure into candidate review evidence.

mod commitments;
mod compilation;
mod custody;
mod error;
mod review_cache;
mod review_set;
mod semantic_bindings;

pub(crate) use commitments::build_observation_commitment;
pub(crate) use compilation::{
    CandidateCompilation, PackageSourceBindings, SemanticBindingReview,
    compile_resolved_package_reviews_reusing,
};
pub(crate) use custody::verify_transitive_source_custody;
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use review_set::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
pub(crate) use semantic_bindings::ConsumerScopedSemanticBindingReviewInput;
