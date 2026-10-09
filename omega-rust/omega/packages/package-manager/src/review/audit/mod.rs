//! Assemble bounded audit inputs and derive deterministic review guidance.

mod input;
mod source_diff;
mod triage;

pub(crate) use source_diff::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits,
    render_package_source_patch};
pub(crate) use triage::triage_review_update_records;
pub(crate) use triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install};
