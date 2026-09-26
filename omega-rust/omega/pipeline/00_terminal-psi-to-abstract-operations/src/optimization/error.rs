use semantic_vocabulary::{MachineId, ObligationId, OperationId};
use terminal_codec::CodecError;
#[derive(Debug)]
pub enum VerifiedPsiOptimizationUnitBuildError {
    Unit(crate::optimization_unit::OptimizationUnitBuildError),
    MissingReconstructedObligation {
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
    },
    MissingAcceptedObligation {
        machine: MachineId,
        operation: OperationId,
        obligation: ObligationId,
    },
    PropositionCodec(CodecError),
    FactIndex(crate::optimization_unit::AcceptedObligationFactIndexError),
    ProofQuestionIndex(crate::optimization_unit::ProofQuestionIndexError),
    OwnershipFrontierFactIndex(crate::optimization_unit::OwnershipFrontierFactIndexError),
    MissingStructuralCatalogMachine(MachineId),
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralFrontier {
        machine: MachineId,
        site: crate::optimization_unit::OwnershipFrontierSite,
    },
}

impl std::fmt::Display for VerifiedPsiOptimizationUnitBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot construct verified Psi optimization unit: {self:?}"
        )
    }
}

impl std::error::Error for VerifiedPsiOptimizationUnitBuildError {}

impl From<crate::optimization_unit::OptimizationUnitBuildError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: crate::optimization_unit::OptimizationUnitBuildError) -> Self {
        Self::Unit(error)
    }
}

impl From<CodecError> for VerifiedPsiOptimizationUnitBuildError {
    fn from(error: CodecError) -> Self {
        Self::PropositionCodec(error)
    }
}

impl From<crate::optimization_unit::AcceptedObligationFactIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: crate::optimization_unit::AcceptedObligationFactIndexError) -> Self {
        Self::FactIndex(error)
    }
}

impl From<crate::optimization_unit::OwnershipFrontierFactIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: crate::optimization_unit::OwnershipFrontierFactIndexError) -> Self {
        Self::OwnershipFrontierFactIndex(error)
    }
}

impl From<crate::optimization_unit::ProofQuestionIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: crate::optimization_unit::ProofQuestionIndexError) -> Self {
        Self::ProofQuestionIndex(error)
    }
}
