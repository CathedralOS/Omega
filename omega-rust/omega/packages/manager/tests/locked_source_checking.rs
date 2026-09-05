//! Recovered project policy never substitutes for a fresh compiler analysis.

use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use package_manager::operations::{
    CheckLockedSourcesError, LockedSourceRecoveryOptions, RecoverLockedSourcesError,
    check_locked_sources,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use package_manager::resolution::package_compilation_inputs;
use package_manager::review::compile_resolved_package_reviews;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::{Path, PathBuf};
use target::TargetProfile;

#[path = "locked_source_checking/generated.rs"]
mod generated;
#[path = "locked_source_checking/ordinary.rs"]
mod ordinary;
#[path = "locked_source_recovery/support.rs"]
mod support;
use support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
