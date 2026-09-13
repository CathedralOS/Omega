use build_evaluation::{BuildFilesystemObservedByteRegionKind, BuildObservationClass};
use package_evidence::ledger::{
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
};
use package_evidence::record::{
    CheckedPackageReviewProjection, PackageReviewCallableRole, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCheckedServiceReach,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewPropositionEvidence, PackageReviewRepresentationTcbKind,
    PackageReviewSourceLocationRole,
};
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolveWorkspacePackageClosureError, ResolvedPackageSourceClosure,
    resolve_workspace_package_closure_with_storage,
};
use package_manager::resolution::source::ResolvePackageSourceError;
use package_manager::review::{
    CompileResolvedPackageReviewsError, PackageSourceVerificationPhase, PackageTriageDisposition,
    PackageTriageReason, ReviewOnlyCapabilityConflictLimits, assemble_initial_source_review,
    assemble_update_source_review, compare_review_only_initial_capabilities,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    triage_initial_install, triage_review_update, triage_update_without_admission_baseline,
};
use package_source::{
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
