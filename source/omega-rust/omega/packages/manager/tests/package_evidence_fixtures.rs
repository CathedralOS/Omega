use omega_build_evaluation::{BuildFilesystemObservedByteRegionKind, BuildObservationClass};
use omega_package_evidence::ledger::{
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
};
use omega_package_evidence::record::{
    CheckedPackageReviewProjection, PackageReviewCallableRole, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCheckedServiceReach,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewPropositionEvidence, PackageReviewRepresentationTcbKind,
    PackageReviewSourceLocationRole,
};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolveExternalLocalPackageClosureError,
    ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure_with_storage,
    resolve_workspace_package_closure_with_storage,
};
use omega_package_manager::resolution::source::ResolvePackageSourceError;
use omega_package_manager::review::{
    CompileResolvedPackageReviewsError, PackageSourceVerificationPhase, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyBaselineCapsule, ReviewOnlyBaselineDirectory,
    ReviewOnlyBaselineFileError, ReviewOnlyBaselineLimits, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError, ReviewOnlyCapabilityConflictLimits,
    assemble_initial_source_review, assemble_update_source_review,
    assemble_update_source_review_from_baseline, compare_review_only_capabilities,
    compare_review_only_capabilities_from_baseline, compare_review_only_initial_capabilities,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    triage_initial_install, triage_review_update, triage_review_update_from_baseline,
    triage_update_without_admission_baseline,
};
use omega_package_source::{
    ExternalSourceContext, LocalSourceLimits, SourceLineage, SourceRelativePath,
    SourceResolveError, SourceResolverStorage,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "package_evidence_fixtures/compiler_review_evidence.rs"]
mod compiler_review_evidence;
#[path = "package_evidence_fixtures/root_role_review_packet.rs"]
mod root_role_review_packet;
#[path = "package_evidence_fixtures/snapshot_tampering.rs"]
mod snapshot_tampering;
#[path = "package_evidence_fixtures/support.rs"]
mod support;

use support::*;
