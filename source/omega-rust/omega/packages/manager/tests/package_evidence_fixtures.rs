use omega_build_evaluation::{BuildFilesystemObservedByteRegionKind, BuildObservationClass};
use omega_package_evidence::evidence::{
    CheckedPackageReviewProjection, PackageReviewCallableRole, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCheckedServiceReach,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewPropositionEvidence, PackageReviewRepresentationAbiCommitment,
    PackageReviewRepresentationMechanism, PackageReviewSourceLocationRole,
};
use omega_package_evidence::obligations::{
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
};
use omega_package_manager::graph::{
    PackageSourceClosureLimits, ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    resolve_workspace_package_closure_with_storage,
};
use omega_package_manager::review::{
    CompileResolvedPackageReviewsError, PackageSourceVerificationPhase, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyCapabilityConflictLimits,
    assemble_initial_source_review, assemble_update_source_review,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities,
    compare_review_only_capabilities_from_baseline, compile_resolved_package_reviews,
    triage_initial_install, triage_review_update, triage_review_update_from_baseline,
    triage_update_without_admission_baseline,
};
use omega_package_manager::sources::ResolvePackageSourceError;
use omega_package_source::{
    LocalSourceLimits, SourceLineage, SourceRelativePath, SourceResolveError, SourceResolverStorage,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "package_evidence_fixtures/compiler_review_evidence.rs"]
mod compiler_review_evidence;
#[path = "package_evidence_fixtures/snapshot_tampering.rs"]
mod snapshot_tampering;
#[path = "package_evidence_fixtures/support.rs"]
mod support;

use support::*;
