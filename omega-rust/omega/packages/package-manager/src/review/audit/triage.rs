//! Deterministic source and provenance triage dispositions.

mod decision;
mod render;

pub(crate) use decision::triage_review_update_records;
pub(crate) use decision::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline};
pub(crate) use render::TriageRenderError;
