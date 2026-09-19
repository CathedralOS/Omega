//! Select, lower, and verify a machine for inspection. Grants no native authority.
//!
//! Source preparation follows the compile command rather than a focused-file
//! shortcut. A root beside a `build.omg` (or an `omega.lock`) is a package
//! project: the package manager resolves its declared closure and checks each
//! prerequisite build before its consumer, retaining generated source under
//! the producer's custody. Acquisition-only compiler inputs cannot provide
//! that handoff, even if a helper happens to have usable physical source.
//! A standalone root keeps the direct, optionally targetless check unchanged.
//! Inspection stops at the checked program and neither admits trust nor
//! realizes native output. Candidate checking is shared with `--check`, but
//! trust admission and native acceptance/publication remain separate operations.

pub mod evidence;

use compiler::{CheckedCompileRequest, CompileOptions};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use package_manager::operations::LocalProjectPreparationOptions;
use std::path::PathBuf;
use terminal_psi::TerminalModule;

pub struct InspectTerminalRequest {
    pub root_path: PathBuf,
    pub machine: String,
    pub target_name: Option<String>,
}

pub struct TerminalInspection {
    pub module: TerminalModule,
    pub fixed_fuel: evidence::FixedFuel,
}

#[derive(Debug)]
pub enum InspectTerminalError {
    Diagnostics(Vec<Diagnostic>),
    Preparation(packages::PrepareLocalProjectError),
    Review(package_manager::review::CompileResolvedPackageReviewsError),
    Lowering {
        machine: String,
        error: checked_trees_to_lowered_psi::LoweringError,
    },
    Evidence {
        machine: String,
        error: evidence::InspectionError,
    },
}

impl std::fmt::Display for InspectTerminalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                for (index, diagnostic) in diagnostics.iter().enumerate() {
                    if index != 0 {
                        writeln!(formatter)?;
                    }
                    write!(formatter, "{diagnostic}")?;
                }
                Ok(())
            }
            Self::Preparation(error) => write!(formatter, "{error}"),
            Self::Review(error) => write!(formatter, "cannot check inspection project: {error}"),
            Self::Lowering { machine, error } => write!(
                formatter,
                "cannot lower terminal machine `{machine}`: {error}"
            ),
            Self::Evidence { machine, error } => write!(
                formatter,
                "cannot inspect terminal machine `{machine}`: {error}"
            ),
        }
    }
}

impl std::error::Error for InspectTerminalError {}

pub fn inspect_terminal(
    request: &InspectTerminalRequest,
) -> Result<TerminalInspection, InspectTerminalError> {
    let checked = check_inspection_sources(request)?;
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, &request.machine).map_err(
        |error| InspectTerminalError::Lowering {
            machine: request.machine.clone(),
            error,
        },
    )?;
    compiler::validate_lowered_ieee_float_comparison_custody(&checked, &lowered)
        .map_err(InspectTerminalError::Diagnostics)?;
    let fixed_fuel =
        evidence::inspect(&lowered.semantic_module, &lowered.proof_bundle).map_err(|error| {
            InspectTerminalError::Evidence {
                machine: request.machine.clone(),
                error,
            }
        })?;
    Ok(TerminalInspection {
        module: lowered.semantic_module,
        fixed_fuel,
    })
}

/// Check package sources through the manager's dependency-first candidate
/// pipeline. The prepared entry lives in an immutable resolver snapshot, so
/// build staging is placed beside the authored root exactly as the compile
/// command places it; the compiler's default of `<entry>/../build` would
/// otherwise fall inside that snapshot. Package preparation needs an exact
/// target, so the invocation resolves one first: an explicit `--target`,
/// else the compiler host's catalogued profile. A host that owns none
/// reports that absence as an ordinary diagnostic rather than panicking.
/// With the profile resolved, the checked compile keeps it so package
/// inputs and target attachments agree; a standalone root without a target
/// stays targetless, as before.
fn check_inspection_sources(
    request: &InspectTerminalRequest,
) -> Result<compiler::CheckedCompilation, InspectTerminalError> {
    let target = crate::invocation_target_profile(request.target_name.as_deref())
        .map_err(|diagnostic| InspectTerminalError::Diagnostics(vec![diagnostic]))?;
    let prepared = packages::prepare_local_project(
        &request.root_path,
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .map_err(InspectTerminalError::Preparation)?;
    let Some(prepared) = prepared else {
        return compiler::compile_to_checked(CheckedCompileRequest::new(
            &request.root_path,
            request.target_name.as_deref(),
        ))
        .map_err(InspectTerminalError::Diagnostics);
    };
    let build_dir = CompileOptions {
        root_path: request.root_path.clone(),
        build_dir: None,
        target_name: None,
    }
    .build_dir();
    packages::check_prepared_local_project_for_inspection(prepared, &build_dir, target)
        .map_err(InspectTerminalError::Review)
}
