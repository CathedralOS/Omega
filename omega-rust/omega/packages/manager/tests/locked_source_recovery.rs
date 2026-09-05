//! A recovered accepted lock supplies pins, never live source or review custody.

use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use omega_package_manager::operations::{
    LockedSourceRecoveryOptions, RecoverLockedSourcesError, recover_locked_sources,
};
use omega_package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use omega_package_manager::resolution::package_compilation_inputs;
use omega_package_manager::review::compile_resolved_package_reviews;
use omega_package_source::git::resolution::GitExactRevisionAcquisition;
use omega_package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use omega_target::TargetProfile;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "locked_source_recovery/git.rs"]
mod git;
#[path = "locked_source_recovery/local.rs"]
mod local;
#[path = "locked_source_recovery/support.rs"]
mod support;
#[path = "locked_source_recovery/workspace.rs"]
mod workspace;
use support::*;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
