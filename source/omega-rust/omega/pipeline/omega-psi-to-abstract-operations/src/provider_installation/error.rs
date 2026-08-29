use crate::artifact::ArtifactLoweringError;
use crate::shared::*;

#[derive(Debug)]
pub enum ProviderInstallationError {
    ArtifactReplay(ArtifactLoweringError),
    PlanReplayMismatch,
    InvalidLoweredCatalog,
    MissingSelectedProvider {
        boundary: psi_core::BoundaryMachineId,
    },
    SelectedProviderMismatch {
        boundary: psi_core::BoundaryMachineId,
    },
    AmbiguousSelectedProvider {
        boundary: psi_core::BoundaryMachineId,
    },
    PsiAdmission(psi_terminal_interpreter::ProviderInstallationError),
    TerminalIdentityMismatch,
    InstalledUnitCallReplayMismatch {
        caller: MachineId,
        operation: OperationId,
        boundary: psi_core::BoundaryMachineId,
    },
}
