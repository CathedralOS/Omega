use crate::artifact_admission::ArtifactLoweringError;
use semantic_vocabulary::{InvocationSite, MachineId, OperationId};

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
    PsiAdmission(terminal_installation::ProviderInstallationError),
    TerminalIdentityMismatch,
    InstalledContentClaimMismatch {
        caller: MachineId,
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        claim: semantic_vocabulary::ClaimId,
        reason: &'static str,
    },
    InstalledCallReplayMismatch {
        caller: MachineId,
        invocation_site: InvocationSite,
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
}
