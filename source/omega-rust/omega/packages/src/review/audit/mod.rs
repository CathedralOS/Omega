//! Assemble bounded audit inputs and derive deterministic review guidance.

mod input;
mod source_diff;
mod triage;

pub(crate) use input::assemble_update_source_review_records;
pub use input::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewInput,
    PackageSourceReviewLimits, PackageSourceReviewRenderError, assemble_initial_source_review,
    assemble_update_source_review,
};
pub use source_diff::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
pub use triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
pub(crate) use triage::{apply_root_role_change, triage_review_update_records};
