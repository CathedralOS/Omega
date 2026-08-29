//! Source-custody/evidence joins and the bounded advisory reviewer boundary.

mod assembly;
mod error;
mod input;
mod invocation;
mod protocol;

pub(crate) use assembly::assemble_update_source_review_records;
pub use assembly::{assemble_initial_source_review, assemble_update_source_review};
pub use error::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewRenderError,
};
pub use input::{PackageSourceReviewInput, PackageSourceReviewLimits};
pub use invocation::invoke_package_advisory_review;
pub use protocol::{
    PackageAdvisoryRecommendation, PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome,
    PackageAdvisoryReviewOutput, PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest,
    PackageAdvisoryReviewer,
};
