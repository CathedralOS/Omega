#![forbid(unsafe_code)]

//! Project Build orchestration independent of CLI parsing and process exits.
//!
//! `build.rs` owns output admission, project preparation, product requests and
//! publication. Project compilation admits a package batch, invokes the compiler,
//! and returns its outcomes to package-owned identity reconciliation and disposal.
//! `build_execution.rs` schedules dependency Builds, joins generated source and
//! waits for demanded components before their consumers. `package_commands.rs`
//! schedules admitted target reviews before package-owned publication. The
//! review dependency loop and audit target/session lifecycle also live here.
//! Terminal Build/helper suspension remains separate migration work.

mod build;
mod build_directory;
mod build_execution;
mod check_project;
mod native_project;
mod package_commands;
mod package_compilation;
mod package_inspection;
mod package_review;
mod products;
mod project_compilation;
pub mod publication;

#[cfg(test)]
mod build_boundary;

pub use build::build;
pub use check_project::{
    CheckPreparedLocalProjectError, CheckedProjectTargetOutcome, PreparedLocalProjectCheckRequest,
    ProjectInspectionError, check_prepared_local_project_for_targets,
    inspect_prepared_local_project,
};
pub use native_project::{
    CompilePreparedLocalProjectNativeError, CompiledProjectTargetOutcome,
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native,
    compile_prepared_local_project_for_native_targets,
};
pub use package_commands::execute_package_command;
pub use package_compilation::{compile_packages, join_dependency_builds, prepare_package};
pub use package_inspection::inspect_packages;

use artifacts::compile_timings::CompileTimings;
use compiler::{CompileReport, OptimizationRollback};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum ProjectProduct {
    Check,
    /// A named machine's Terminal description, with no native realization.
    Inspection {
        machine: String,
    },
    Native {
        destination: NativeDestination,
        private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    },
}

/// Retention transfers the in-memory product to a consumer such as `run`,
/// which needs Terminal comparison before publication consumes the artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeDestination {
    Retained,
    Published,
}

/// Target identity and output placement are explicit, not inferred from an
/// unrelated options field. A batch uses target subdirectories even if the
/// caller's explicit set canonicalizes to one profile.
#[derive(Clone)]
pub enum BuildTargets {
    Host,
    Single(target::TargetProfile),
    Batch(target::ExplicitTargetSet),
}

#[derive(Clone)]
pub struct BuildRequest {
    pub root_path: PathBuf,
    pub build_directory: Option<PathBuf>,
    pub targets: BuildTargets,
    pub product: ProjectProduct,
    pub timings: bool,
    pub offline: bool,
    /// Recheck the root under retained pins without accepted restricted grants.
    pub verify: bool,
    /// Enforced after transaction recovery and project discovery.
    pub require_package_project: bool,
    pub optimization_rollback: OptimizationRollback,
    pub build_snapshot: Option<compiler::BuildSnapshotRequest>,
}

pub struct BuildOutcome {
    pub output: compiler::CompilationOutput,
    pub timings: CompileTimings,
    pub executable_path: Option<PathBuf>,
}

/// A check or native outcome ready for compilation-command presentation.
pub struct BuildReport {
    pub report: CompileReport,
    pub timings: CompileTimings,
    /// Present only after native publication succeeds.
    pub executable_path: Option<PathBuf>,
}

impl BuildOutcome {
    pub fn into_report(self) -> Result<BuildReport, BuildError> {
        let report = self
            .output
            .into_report()
            .map_err(|error| BuildError::Diagnostics(error.into_diagnostics()))?;
        Ok(BuildReport {
            report,
            timings: self.timings,
            executable_path: self.executable_path,
        })
    }
}

pub struct TargetBuildOutcome {
    pub target: target::TargetProfile,
    pub result: Result<BuildOutcome, BuildError>,
}

#[derive(Debug)]
pub enum BuildError {
    Diagnostics(Vec<Diagnostic>),
    Preparation(packages::PrepareLocalProjectError),
    PackageCheck(CheckPreparedLocalProjectError),
    PackageNative(CompilePreparedLocalProjectNativeError),
    Inspection(ProjectInspectionError),
    Publication(String),
    CheckWorkspace(std::io::Error),
    BuildDirectory(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                diagnostics::write_diagnostics(formatter, diagnostics)
            }
            Self::Preparation(error) => write!(formatter, "{error}"),
            Self::PackageCheck(error) => write!(formatter, "{error}"),
            Self::PackageNative(error) => write!(formatter, "{error}"),
            Self::Inspection(error) => {
                write!(formatter, "cannot check inspection project: {error}")
            }
            Self::Publication(error) => write!(formatter, "{error}"),
            Self::CheckWorkspace(error) => {
                write!(formatter, "cannot create the check workspace: {error}")
            }
            Self::BuildDirectory(conflict) => write!(formatter, "{conflict}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl BuildError {
    /// Preserve structured compiler refusals when an execution consumer needs
    /// their exceeded-provision classification, not just display text.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics)
            | Self::PackageNative(CompilePreparedLocalProjectNativeError::Native(diagnostics)) => {
                diagnostics
            }
            error => vec![Diagnostic::error(error.to_string())],
        }
    }

    /// Only compilation refusals consisting entirely of exceeded private
    /// provisions are Incomplete; preparation and publication refusals reject.
    pub fn outcome(&self) -> compiler::CompilerRequestOutcome {
        match self {
            Self::Diagnostics(diagnostics)
            | Self::PackageNative(CompilePreparedLocalProjectNativeError::Native(diagnostics)) => {
                compiler::CompilerRequestOutcome::of_refusal(diagnostics)
            }
            _ => compiler::CompilerRequestOutcome::Reject,
        }
    }
}
