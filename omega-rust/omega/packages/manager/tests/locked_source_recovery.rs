//! A recovered accepted lock supplies pins, never live source or review custody.

use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use package_manager::operations::{
    LockedSourceRecoveryOptions, RecoverLockedSourcesError, recover_locked_sources,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use package_manager::resolution::package_compilation_inputs;
use package_manager::review::compile_resolved_package_reviews;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::fs;
use std::path::{Path, PathBuf};
use target::TargetProfile;

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
