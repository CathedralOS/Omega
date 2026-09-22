//! Compact acceptance changes use real, independently compiled candidate audits.

use package_evidence::record::PackagePolicyRowKind;
use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageSourceClosureLimits,
    ResolvedPackageSourceClosure, resolve_external_local_project_closure,
};
use package_manager::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeKind, PackagePolicyChangeLimits,
    compare_package_policy_changes, compile_resolved_package_reviews,
};
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::{Path, PathBuf};
use target::TargetProfile;

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
#[path = "package_policy_changes/history.rs"]
mod history;
#[path = "package_policy_changes/initial.rs"]
mod initial;
#[path = "package_policy_changes/operation.rs"]
mod operation;
#[path = "package_policy_changes/replacements.rs"]
mod replacements;
use crate::locked_source_recovery::support::*;
use fixtures::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
