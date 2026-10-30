//! Select an admitted checked provider for a domain-ending invocation.
//! The provider executes before host completion; boundary identity alone never
//! authorizes simulating the completion or manufacturing its result custody.

use crate::errors::TerminalInterpretError;
use crate::program::AdmittedTerminalProgram;
use semantic_vocabulary::{BoundaryMachineId, MachineId};

pub(crate) fn checked_provider_callee(
    program: &AdmittedTerminalProgram<'_>,
    supply: Option<crate::effects::AdmittedBoundarySupply>,
    boundary: BoundaryMachineId,
) -> Result<Option<MachineId>, TerminalInterpretError> {
    match supply {
        Some(crate::effects::AdmittedBoundarySupply::CheckedProvider(callee)) => Ok(Some(callee)),
        None if program.has_provider(&boundary) => Err(
            TerminalInterpretError::ProviderInstallationMissing(boundary),
        ),
        _ => Ok(None),
    }
}
