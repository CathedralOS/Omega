//! Compiler review of every package in one resolved immutable closure.

mod compilation;
mod custody;
mod error;
pub(crate) mod inputs;
mod ledger;
mod model;
mod session;

pub use compilation::compile_resolved_package_reviews;
pub use error::CompileResolvedPackageReviewsError;
pub use inputs::{package_compilation_inputs, package_compilation_inputs_for};
pub use model::{
    CompilerExecutableVerificationPhase, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
