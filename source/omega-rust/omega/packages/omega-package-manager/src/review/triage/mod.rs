//! Deterministic source and provenance triage dispositions.

mod decision;
mod render;

pub use decision::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
pub(crate) use decision::{apply_root_role_change, triage_review_update_records};
pub use render::TriageRenderError;
