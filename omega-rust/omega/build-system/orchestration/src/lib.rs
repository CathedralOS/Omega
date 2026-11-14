/*

Project Build orchestration and package-workflow coordinator.

This crate sits between application entry points (`omega` CLI, test harnesses,
and embedders) and the lower-level `compiler`, `native_compiler`, and
`package_manager` crates. It owns the execution loops that step `build.omg`
programs, resolve dependency graphs at readiness pauses, schedule multi-target
compilation, and publish final artifacts, without printing to the console or
exiting the process.

The crate is organized into three subsystems around `build.rs` and `build_session.rs`:
- `project/`: turns a `BuildRequest` into compiler requests (`products.rs`),
  runs admitted package batches (`project_compilation.rs`), guards the output
  directory against concurrent or aliased roots (`build_directory.rs`), and
  publishes completed native artifacts (`publication.rs`).
- `package/`: drives `omega packages install` and `omega packages update` review loops
  (`package_commands.rs`), `omega packages audit` (`package_inspection.rs`),
  dependency-first review passes (`package_review.rs`), and multi-package
  compilation (`package_compilation.rs`).
- `compile/`: steps `build.omg` activations and dependency frontiers
  (`build_execution.rs` and `source_resolution/`), runs one-shot standalone
  compilations (`source_compilation.rs`), and completes post-Terminal native
  products and scalar constant demands (`native_completion.rs`).

Product and target vocabulary:
- `ProjectProduct` selects where the pipeline stops: `Check` (proof and type
  checking only), `Terminal` (retains the canonical Terminal Psi artifact in
  memory), `Inspection { machine }` (extracts one machine's Terminal Psi module),
  or `Native { destination, private_entry_ram }`.
- `NativeDestination` distinguishes `Published` (writes the executable or `.app`
  bundle to disk), `Retained` (keeps the native product in memory so `omega run
  --both` can run Terminal Psi comparison before publishing), and
  `RuntimeAdmission` (stops after provider and entry admission without emitting
  machine code, for `omega run --engine psi`).
- `BuildTargets` keeps target selection explicit: `Host` (the current host
  profile), `Single` (one named profile in the root build directory), or `Batch`
  (an explicit target set, where each target gets its own `<build_dir>/<target>`
  subdirectory even if the set has length 1).
- `BuildError::outcome` classifies compilation refusals through
  `CompilerRequestOutcome::of_refusal`: only a refusal consisting entirely of
  exceeded private provisions returns `Incomplete`; preparation, directory, and
  publication errors always return `Reject`.

*/

/*

Why orchestration owns the execution loop instead of `compiler` or `package_manager`:

A `build.omg` program can request external packages, query product metadata, or
pause at a provider readiness boundary while waiting for a dependency's Checked
Terminal Psi implementation to be compiled. If `compiler` stepped `build.omg`
directly, the compiler would have to depend on package acquisition and source
staging; if `package_manager` owned a `CompilationSession`, package policy would
be coupled to compiler state. Instead, `BuildSession` holds `CompilationSession`
and `PackageBuildSession` side by side here, and `orchestration` drives the
alternation between them.

*/

#![forbid(unsafe_code)]

mod build;
mod build_session;
mod compile;
mod package;
mod project;
pub use compile::build_execution::execute_build as execute_build_activation;
pub use compile::native_completion::complete_product;
pub use compile::source_compilation::{
    check_source, check_source_keeping_typed_frontier, compile_sources, produce_source,
};
pub use project::publication;

#[cfg(test)]
mod build_boundary;

pub use build::build;
pub use build_session::BuildSession;
pub use compile::build_execution::source_resolution::{
    ProjectSourceRequest, ProjectSourceResolutionError, ResolvedPackageBuild,
    ResolvedProjectBuilds, resolve_project_sources,
};
pub use package::package_commands::execute_package_command;
pub use package::package_compilation::{compile_packages, prepare_package};
pub use package::package_inspection::inspect_packages;

use artifacts::compile_timings::CompileTimings;
use compiler::{CompileReport, OptimizationRollback};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum ProjectProduct {
    Check,
    Terminal,
    Inspection {
        machine: String,
    },
    Native {
        destination: NativeDestination,
        private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
        installation_assumptions: Vec<compilation_products::InstallationEnvironmentAssumption>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeDestination {
    Retained,
    Published,
    RuntimeAdmission, // Stop after provider and entry admission without emitting a native image.
}

impl NativeDestination {
    const fn image_demand(self) -> compilation_products::NativeImageDemand {
        match self {
            Self::Retained | Self::Published => compilation_products::NativeImageDemand::Emit,
            Self::RuntimeAdmission => compilation_products::NativeImageDemand::RuntimeAdmissionOnly,
        }
    }
}

#[derive(Clone)]
pub enum BuildTargets {
    Host,
    Single(target::TargetProfile),
    Batch(target::ExplicitTargetSet), // Always uses per-target subdirectories.
}

#[derive(Clone)]
pub struct BuildRequest {
    pub root_path: PathBuf,
    pub build_directory: Option<PathBuf>,
    pub targets: BuildTargets,
    pub product: ProjectProduct,
    pub timings: bool,
    pub offline: bool,
    pub verify: bool, // Recheck the root under retained pins without accepted restricted grants.
    pub require_package_project: bool, // Enforced after transaction recovery and project discovery.
    pub optimization_rollback: OptimizationRollback,
    pub build_snapshot: Option<compiler::BuildSnapshotRequest>,
}

pub struct BuildOutcome {
    pub output: compiler::CompilationOutput,
    pub timings: CompileTimings,
    pub executable_path: Option<PathBuf>,
}

pub struct BuildReport {
    pub report: CompileReport,
    pub timings: CompileTimings,
    pub executable_path: Option<PathBuf>, // Present only after native publication succeeds.
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
    ProjectCompilation(packages::ProjectCompilationError),
    Publication(String),
    CheckWorkspace(std::io::Error),
    BuildDirectory(String),
}

impl From<Vec<Diagnostic>> for BuildError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                diagnostics::write_diagnostics(formatter, diagnostics)
            }
            Self::Preparation(error) => write!(formatter, "{error}"),
            Self::ProjectCompilation(error) => {
                write!(formatter, "cannot compile admitted project: {error}")
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
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostics(diagnostics) => diagnostics,
            error => vec![Diagnostic::error(error.to_string())],
        }
    }

    pub fn outcome(&self) -> compiler::CompilerRequestOutcome {
        match self {
            Self::Diagnostics(diagnostics) => {
                compiler::CompilerRequestOutcome::of_refusal(diagnostics)
            }
            _ => compiler::CompilerRequestOutcome::Reject,
        }
    }
}
