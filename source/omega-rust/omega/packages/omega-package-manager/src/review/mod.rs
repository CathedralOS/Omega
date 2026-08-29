//! Compiler review, candidate comparison, advisory triage, and root policy.

pub(crate) mod advisory_review;
pub(crate) mod baseline;
pub mod comparison;
pub(crate) mod compilation_inputs;
pub(crate) mod compiler_review;
pub(crate) mod evidence;
pub(crate) mod reconstruction_question;
pub(crate) mod review_set_validation;
pub(crate) mod review_triage;
pub(crate) mod root_policy;
pub(crate) mod source_diff;

pub use advisory_review::{
    PackageAdvisoryRecommendation, PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome,
    PackageAdvisoryReviewOutput, PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest,
    PackageAdvisoryReviewer, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    assemble_initial_source_review, assemble_update_source_review, invoke_package_advisory_review,
};
pub use baseline::{
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory, ReviewOnlyBaselineError,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyBaselinePackage,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities_from_baseline,
    triage_review_update_from_baseline,
};
pub use comparison::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictChange, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictRenderError, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts, ReviewSetRole, compare_review_only_capabilities,
};
pub use compilation_inputs::{package_compilation_inputs, package_compilation_inputs_for};
pub use compiler_review::{
    CompileResolvedPackageReviewsError, CompilerExecutableVerificationPhase,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    compile_resolved_package_reviews,
};
pub use evidence::{
    ReviewOnlyCanonicalRow, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlySourceConsumptionCommitment,
};
pub use reconstruction_question::{
    CanonicalPackageReconstructionEntry, CanonicalPackageReconstructionQuestion,
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionFingerprint,
    CanonicalPackageReconstructionQuestionLimits, PACKAGE_RECONSTRUCTION_QUESTION_ENCODING_VERSION,
};
pub use review_triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
pub use root_policy::{
    ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDirectory, ReviewOnlyRootPolicyDisposition,
    ReviewOnlyRootPolicyFileError, ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyNameError,
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, recover_review_only_root_policy_resolution,
    resolve_review_only_root_policy_decisions,
};
pub use source_diff::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
