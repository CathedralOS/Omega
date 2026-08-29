//! Compiler review, candidate comparison, advisory triage, and root policy.

pub(crate) mod baseline;
pub(crate) mod closure;
pub mod comparison;
pub(crate) mod compiler_handoff;
pub(crate) mod compiler_review;
pub(crate) mod evidence;
pub(crate) mod policy;
pub(crate) mod reconstruction;
pub(crate) mod source_patch;
pub(crate) mod source_review;
pub(crate) mod source_triage;

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
pub use compiler_handoff::{package_compilation_inputs, package_compilation_inputs_for};
pub use compiler_review::{
    CompileResolvedPackageReviewsError, CompilerExecutableVerificationPhase,
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
    compile_resolved_package_reviews,
};
pub use evidence::{
    ReviewOnlyCanonicalRow, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlySourceConsumptionCommitment,
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
pub use source_patch::{
    PackageSourcePatch, PackageSourcePatchError, PackageSourcePatchLimits, PackageSourcePatchSide,
    render_package_source_patch,
};
pub use source_review::{
    PackageAdvisoryRecommendation, PackageAdvisoryReviewError, PackageAdvisoryReviewOutcome,
    PackageAdvisoryReviewOutput, PackageAdvisoryReviewOutputError, PackageAdvisoryReviewRequest,
    PackageAdvisoryReviewer, PackageSourceReviewCustodyRole, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, PackageSourceReviewRenderError,
    assemble_initial_source_review, assemble_update_source_review, invoke_package_advisory_review,
};
pub use source_triage::{
    CompilerReviewTriage, PackageTriageDecision, PackageTriageDisposition, PackageTriageReason,
    TriageRenderError, triage_initial_install, triage_review_update,
    triage_update_without_admission_baseline,
};
