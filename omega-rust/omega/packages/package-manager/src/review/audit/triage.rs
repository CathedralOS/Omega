//! Deterministic source and provenance triage dispositions.

mod decision;
mod render;

pub(crate) use decision::triage_review_update_records;
pub use decision::{
};
pub(crate) use decision::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    triage_initial_install};
pub(crate) use render::TriageRenderError;
