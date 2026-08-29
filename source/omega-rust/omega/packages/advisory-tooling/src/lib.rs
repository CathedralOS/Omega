#![forbid(unsafe_code)]

//! Optional model-facing package source-review tooling.
//!
//! The package manager publishes deterministic source and capability changes;
//! this separate tooling crate owns prompts, response schemas, and reviewer
//! invocation. Its availability and output cannot affect package acceptance.

mod invocation;
mod protocol;

pub use invocation::invoke_package_advisory_review;
pub use protocol::{
    PackageAdvisoryRecommendation, PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome,
    PackageAdvisoryReviewOutput, PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest,
    PackageAdvisoryReviewer,
};
