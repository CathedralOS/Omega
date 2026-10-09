//! Compile one resolved immutable closure into candidate review evidence.

mod commitments;
mod compilation;
mod custody;
mod error;
mod evidence;
mod review_cache;
mod review_set;
mod rows;
mod semantic_bindings;
pub(crate) mod validation;

pub(crate) use commitments::build_observation_commitment;
pub use compilation::{
};
pub(crate) use compilation::{
    CandidateCompilation, SemanticBindingReview,
    compile_resolved_package_reviews_reusing};
pub(crate) use compilation::{ CandidateSourcePreparation};
pub(crate) use custody::verify_transitive_source_custody;
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use evidence::PackageReviewEvidence;
pub use review_set::{
};
pub(crate) use review_set::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase};
pub(crate) use rows::ReviewOnlySourceConsumptionCommitment;
pub use semantic_bindings::{
};
pub(crate) use semantic_bindings::{
    ConsumerScopedSemanticBindingReviewInput};
