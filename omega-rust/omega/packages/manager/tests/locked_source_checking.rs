//! Recovered project policy never substitutes for a fresh compiler analysis.

use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use omega_package_manager::operations::{
    CheckLockedSourcesError, LockedSourceRecoveryOptions, RecoverLockedSourcesError,
    check_locked_sources,
};
use omega_package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use omega_package_manager::resolution::package_compilation_inputs;
use omega_package_manager::review::compile_resolved_package_reviews;
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use omega_target::TargetProfile;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "locked_source_checking/generated.rs"]
mod generated;
#[path = "locked_source_checking/ordinary.rs"]
mod ordinary;
#[path = "locked_source_recovery/support.rs"]
mod support;
use support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
