//! Source-custody/evidence joins and the bounded advisory reviewer boundary.

mod advisory;
mod assembly;
mod error;
mod input;

pub use advisory::{
    PackageAdvisoryRecommendation, PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome,
    PackageAdvisoryReviewOutput, PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest,
    PackageAdvisoryReviewer, invoke_package_advisory_review,
};
pub(crate) use assembly::assemble_update_source_review_records;
pub use assembly::{assemble_initial_source_review, assemble_update_source_review};
pub use error::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewRenderError,
};
pub use input::{PackageSourceReviewInput, PackageSourceReviewLimits};
