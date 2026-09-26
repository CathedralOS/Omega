//! Recovered project policy never substitutes for a fresh compiler analysis.

use omega::package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use omega::package_manager::operations::{
    CheckLockedSourcesError, LockedSourceRecoveryOptions, RecoverLockedSourcesError,
    check_locked_sources,
};
use omega::package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use omega::package_manager::review::compile_resolved_package_reviews;
use omega::package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::PathBuf;
use target::TargetProfile;

#[path = "locked_source_checking/generated.rs"]
mod generated;
#[path = "locked_source_checking/ordinary.rs"]
mod ordinary;
#[path = "locked_source_checking/restricted_build_grants.rs"]
mod restricted_build_grants;
use crate::locked_source_recovery::support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
