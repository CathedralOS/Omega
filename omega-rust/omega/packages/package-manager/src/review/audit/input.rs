//! Deterministic source-custody and evidence joins for optional review tools.

mod assembly;
mod error;
#[allow(clippy::module_inception)] // This group owns both input assembly and the input record.
mod input;

pub(crate) use assembly::{assemble_initial_source_review, assemble_update_source_review};
pub(crate) use error::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewRenderError};
pub(crate) use input::{PackageSourceReviewInput, PackageSourceReviewLimits};
