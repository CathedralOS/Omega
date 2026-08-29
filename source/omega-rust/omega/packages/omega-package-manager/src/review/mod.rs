//! Compile checked review records, compare candidates, and apply root policy.

pub(crate) mod audit_input;
pub(crate) mod baseline;
pub mod comparison;

pub(crate) mod compilation;
pub(crate) mod policy;
pub(crate) mod reconstruction;
pub(crate) mod records;
pub(crate) mod source_diff;
pub(crate) mod triage;

pub use audit_input::{
    PackageSourceReviewCustodyRole, PackageSourceReviewError, PackageSourceReviewInput,
    PackageSourceReviewLimits, PackageSourceReviewRenderError, assemble_initial_source_review,
    assemble_update_source_review,
};
pub use baseline::{
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory, ReviewOnlyBaselineError,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyBaselinePackage,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities_from_baseline,
    compare_review_only_root_role_from_baseline, triage_review_update_from_baseline,
};
pub use comparison::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictRenderError, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewOnlyRootRoleChange,
    ReviewOnlyRootRoleComparisonError, ReviewOnlyRootRoleContract, ReviewSetRole,
    compare_review_only_capabilities,
};

pub use compilation::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    compile_resolved_package_reviews, package_compilation_inputs, package_compilation_inputs_for,
};
pub use policy::{
    ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyDisposition,
    ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyNameError,
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, recover_review_only_root_policy_resolution,
    resolve_review_only_root_policy_decisions,
};
pub use reconstruction::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits, PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION,
};
pub use records::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
pub use source_diff::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
pub use triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
