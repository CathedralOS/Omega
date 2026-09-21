//! Compile one resolved immutable closure into candidate review evidence.

mod commitments;
mod compilation;
mod custody;
mod error;
mod evidence;
mod review_set;
mod rows;
mod semantic_bindings;
pub(crate) mod validation;

pub(crate) use commitments::{build_observation_commitment, whole_review_commitment};
pub(crate) use compilation::{
    compile_resolved_package_candidate_for_check,
    compile_resolved_package_candidate_for_check_with_checkpoint,
};
pub use compilation::{
    CandidateSourcePreparation, SemanticBindingReview,
    compile_resolved_package_candidate_for_production,
    compile_resolved_package_candidate_for_production_with_checkpoint,
    compile_resolved_package_reviews, compile_resolved_package_reviews_reusing,
    compile_resolved_package_reviews_with_checkpoint,
};
pub(crate) use custody::verify_transitive_source_custody;
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use evidence::PackageReviewEvidence;
pub use review_set::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    ReviewedPackageProductionCandidate,
};
pub use rows::ReviewOnlySourceConsumptionCommitment;
pub use semantic_bindings::{
    ConsumerScopedSemanticBindingReviewInput, SemanticBindingReviewCandidate,
};
