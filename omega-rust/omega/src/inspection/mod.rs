//! Select, lower, and verify a machine for inspection. Grants no native authority.
//!
//! Source preparation follows the compile command rather than a focused-file
//! shortcut. A root beside a `build.omg` (or an `omega.lock`) is a package
//! project: the package manager resolves its declared closure and the checked
//! compile receives that graph as package inputs, so `use alias::module;`
//! binds to the declared dependency instead of a sibling path under the root.
//! A standalone root keeps the direct, optionally targetless check unchanged.
//! Inspection stops at the checked program and neither admits trust nor
//! realizes native output, so the manager's review and acceptance passes that
//! `--check` and `run` add on top of the same prepared closure are not run here.

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
    let checked = compiler::compile_to_checked(checked_compile_request(request)?)
        .map_err(InspectTerminalError::Diagnostics)?;
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

/// Prepare the checked compile the same way `--check` does: the manager decides
/// whether the root is a package project and, if so, supplies the resolved
/// closure. The prepared entry lives in an immutable resolver snapshot, so
/// build staging is placed beside the authored root exactly as the compile
/// command places it; the compiler's default of `<entry>/../build` would
/// otherwise fall inside that snapshot. Package preparation needs an exact
/// target, so the invocation resolves one first: an explicit `--target`,
/// else the compiler host's catalogued profile. A host that owns none
/// reports that absence as an ordinary diagnostic rather than panicking.
/// With the profile resolved, the checked compile keeps it so package
/// inputs and target attachments agree; a standalone root without a target
/// stays targetless, as before.
fn checked_compile_request(
    request: &InspectTerminalRequest,
) -> Result<CheckedCompileRequest<'static>, InspectTerminalError> {
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
        return Ok(CheckedCompileRequest::new(
            &request.root_path,
            request.target_name.as_deref(),
        ));
    };
    let build_dir = CompileOptions {
        root_path: request.root_path.clone(),
        build_dir: None,
        target_name: None,
    }
    .build_dir();
    let (entry_path, package_inputs) = prepared
        .try_into_parts()
        .map_err(InspectTerminalError::Preparation)?;
    let mut checked = CheckedCompileRequest::new(&entry_path, Some(target.target_name()));
    // A packaged binding carrying the compiler-captured canonical Source
    // metadata index is sealed package custody: its build activation runs
    // against a fresh private materialization of the captured inventory,
    // never the shared resolver snapshot, exactly as the manager's review
    // pass and the retained-source `check` rejoin already request. The
    // invocation roster stays empty; required outputs remain the obligations
    // the build registers through `builder.output.require`. A binding without
    // that index has no validated inventory to capture against and keeps the
    // root it names.
    checked.build_snapshot = package_inputs
        .canonical_source_metadata(package_inputs.root())
        .map(|_| compiler::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
    checked.package_inputs = Some(package_inputs);
    checked.build_dir = Some(build_dir);
    Ok(checked)
}
