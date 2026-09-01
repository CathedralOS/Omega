//! Compile one resolved immutable closure into candidate review evidence.

mod commitments;
mod compilation;
mod custody;
mod error;
mod evidence;
mod ledger;
mod model;
mod rows;
mod semantic_bindings;
mod session;
pub(crate) mod validation;

pub(crate) use commitments::{build_observation_commitment, whole_review_commitment};
pub use compilation::{
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    compile_resolved_package_reviews_with_semantic_bindings,
};
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use evidence::PackageReviewEvidence;
pub use model::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
pub use rows::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
pub use semantic_bindings::ConsumerScopedSemanticBindingReviewInput;
