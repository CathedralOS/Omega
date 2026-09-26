//! Deterministic source-custody and evidence joins for optional review tools.

mod assembly;
mod error;
#[allow(clippy::module_inception)] // This group owns both input assembly and the input record.
mod input;

pub use assembly::{assemble_initial_source_review, assemble_update_source_review};
pub use error::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewRenderError,
};
pub use input::{PackageSourceReviewInput, PackageSourceReviewLimits};
