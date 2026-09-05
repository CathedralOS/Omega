//! Compile one resolved immutable closure into candidate review evidence.

mod commitments;
mod compilation;
mod custody;
mod error;
mod evidence;
mod ledger;
mod model;
mod policy;
mod rows;
mod semantic_bindings;
mod session;
pub(crate) mod validation;

pub(crate) use commitments::{build_observation_commitment, whole_review_commitment};
pub use compilation::{
    compile_resolved_package_candidate_for_production,
    compile_resolved_package_candidate_for_production_with_semantic_bindings,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    compile_resolved_package_reviews_with_semantic_bindings,
};
pub(crate) use custody::verify_transitive_source_custody;
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use evidence::PackageReviewEvidence;
pub use model::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    ReviewedPackageProductionCandidate,
};
pub use rows::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
pub use semantic_bindings::{
    ConsumerScopedSemanticBindingReviewInput, SemanticBindingReviewCandidate,
};
