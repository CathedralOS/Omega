//! Deterministic source-custody and evidence joins for optional review tools.

mod assembly;
mod error;
mod input;

pub(crate) use assembly::assemble_update_source_review_records;
pub use assembly::{assemble_initial_source_review, assemble_update_source_review};
pub use error::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewRenderError,
};
pub use input::{PackageSourceReviewInput, PackageSourceReviewLimits};
