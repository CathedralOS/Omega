use crate::artifact::ArtifactLoweringError;
use crate::shared::*;

#[derive(Debug)]
pub enum ProviderInstallationError {
    ArtifactReplay(ArtifactLoweringError),
    PlanReplayMismatch,
    InvalidLoweredCatalog,
    MissingSelectedProvider {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    SelectedProviderMismatch {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    AmbiguousSelectedProvider {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    PsiAdmission(terminal_interpreter::ProviderInstallationError),
    TerminalIdentityMismatch,
    InstalledCallReplayMismatch {
        caller: MachineId,
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
}
