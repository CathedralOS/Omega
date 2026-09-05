//! Complete retained policy changes use real, independently compiled candidates.

use omega_package_evidence::record::PackagePolicyRowKind;
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
    CompilerIssuedPackageReviewSet, PackagePolicyChangeKind, PackagePolicyChangeLimits,
    compare_package_policy_changes, compile_resolved_package_candidate_reviews,
    compile_resolved_package_reviews,
};
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use omega_target::TargetProfile;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "package_policy_changes/authority.rs"]
mod authority;
#[path = "package_policy_changes/decisions.rs"]
mod decisions;
#[path = "package_policy_changes/document.rs"]
mod document;
#[path = "package_policy_changes/fixtures.rs"]
mod fixtures;
#[path = "package_policy_changes/graph.rs"]
mod graph;
#[path = "package_policy_changes/initial.rs"]
mod initial;
#[path = "package_policy_changes/replacements.rs"]
mod replacements;
#[path = "locked_source_recovery/support.rs"]
mod support;
use fixtures::*;
use support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
