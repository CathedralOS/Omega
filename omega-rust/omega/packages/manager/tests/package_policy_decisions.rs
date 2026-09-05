//! Fresh normalized decisions and inert history have separate consumers.

use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
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

#[path = "package_policy_changes/fixtures.rs"]
mod fixtures;
#[path = "package_policy_decisions/helpers.rs"]
mod helpers;
#[path = "package_policy_decisions/history.rs"]
mod history;
#[path = "package_policy_decisions/resolution.rs"]
mod resolution;
#[path = "package_policy_decisions/roles.rs"]
mod roles;
#[path = "locked_source_recovery/support.rs"]
mod support;
use fixtures::*;
use helpers::*;
use support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
const ACCEPT: ReviewOnlyRootPolicyDisposition =
    ReviewOnlyRootPolicyDisposition::AcceptCandidateChange;
const REJECT: ReviewOnlyRootPolicyDisposition =
    ReviewOnlyRootPolicyDisposition::RejectCandidateChange;
