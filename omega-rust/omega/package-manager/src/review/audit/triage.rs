//! Deterministic source and provenance triage dispositions.

mod decision;
mod render;

pub(crate) use decision::triage_review_update_records;
pub use decision::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
pub use render::TriageRenderError;
