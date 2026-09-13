//! Select, lower, and verify a machine for inspection. Grants no native authority.

pub mod evidence;

use compiler::CheckedCompileRequest;
use diagnostics::Diagnostic;
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
    let checked = compiler::compile_to_checked(CheckedCompileRequest::new(
        &request.root_path,
        request.target_name.as_deref(),
    ))
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
