//! Compile one resolved immutable closure into candidate review evidence.

mod commitments;
mod compilation;
mod custody;
mod error;
mod evidence;
pub(crate) mod inputs;
mod ledger;
mod model;
mod rows;
mod session;
pub(crate) mod validation;

pub(crate) use commitments::{build_observation_commitment, whole_review_commitment};
pub use compilation::compile_resolved_package_reviews;
pub use error::CompileResolvedPackageReviewsError;
pub(crate) use evidence::PackageReviewEvidence;
pub use inputs::{package_compilation_inputs, package_compilation_inputs_for};
pub use model::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
pub use rows::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
