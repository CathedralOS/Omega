//! Assemble bounded audit inputs and derive deterministic review guidance.

mod input;
mod source_diff;
mod triage;

pub(crate) use input::{
    assemble_update_source_review};
pub(crate) use source_diff::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits,
    render_package_source_patch};
pub(crate) use triage::triage_review_update_records;
pub(crate) use triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install,
    triage_update_without_admission_baseline};
