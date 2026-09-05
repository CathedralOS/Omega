//! Concrete candidate lock preparation keeps proof and project policy separate.

use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use omega_package_manager::operations::{
    PrepareCandidateLockError, PrepareCandidateLockLimits, prepare_candidate_lock_target,
};
use omega_package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure_with_storage,
};
use omega_package_manager::resolution::package_compilation_inputs;
use omega_package_manager::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits, PackagePolicyChangeSet,
    PackagePolicyDecision, PackagePolicyDecisionLimits, PackagePolicyDecisionResolution,
    ReviewOnlyRootPolicyDisposition, compare_package_policy_changes,
    compile_resolved_package_candidate_reviews, compile_resolved_package_reviews,
    recover_package_policy_decisions, resolve_package_policy_decisions,
};
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use omega_target::TargetProfile;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "candidate_lock_preparation/basic.rs"]
mod basic;
#[allow(dead_code)]
#[path = "package_policy_changes/fixtures.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "package_policy_decisions/helpers.rs"]
mod helpers;
#[path = "candidate_lock_preparation/proofs.rs"]
mod proofs;
#[path = "candidate_lock_preparation/sources.rs"]
mod sources;
#[allow(dead_code)]
#[path = "locked_source_recovery/support.rs"]
mod support;
use fixtures::*;
use helpers::*;
use support::*;

const TARGET: TargetProfile = TargetProfile::LinuxX64;
const ACCEPT: ReviewOnlyRootPolicyDisposition =
    ReviewOnlyRootPolicyDisposition::AcceptCandidateChange;
const REJECT: ReviewOnlyRootPolicyDisposition =
    ReviewOnlyRootPolicyDisposition::RejectCandidateChange;
