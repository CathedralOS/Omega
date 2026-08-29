use crate::shared::*;

#[derive(Debug)]
pub enum VerifiedPsiOptimizationUnitBuildError {
    Unit(omega_optimization_unit::OptimizationUnitBuildError),
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
    FactIndex(omega_optimization_unit::AcceptedObligationFactIndexError),
    ProofQuestionIndex(omega_optimization_unit::ProofQuestionIndexError),
    OwnershipFrontierFactIndex(omega_optimization_unit::OwnershipFrontierFactIndexError),
    MissingStructuralCatalogMachine(MachineId),
    MissingStructuralFrontierMachine(MachineId),
    MissingStructuralFrontier {
        machine: MachineId,
        site: omega_optimization_unit::OwnershipFrontierSite,
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

impl From<omega_optimization_unit::OptimizationUnitBuildError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::OptimizationUnitBuildError) -> Self {
        Self::Unit(error)
    }
}

impl From<CodecError> for VerifiedPsiOptimizationUnitBuildError {
    fn from(error: CodecError) -> Self {
        Self::PropositionCodec(error)
    }
}

impl From<omega_optimization_unit::AcceptedObligationFactIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::AcceptedObligationFactIndexError) -> Self {
        Self::FactIndex(error)
    }
}

impl From<omega_optimization_unit::OwnershipFrontierFactIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::OwnershipFrontierFactIndexError) -> Self {
        Self::OwnershipFrontierFactIndex(error)
    }
}

impl From<omega_optimization_unit::ProofQuestionIndexError>
    for VerifiedPsiOptimizationUnitBuildError
{
    fn from(error: omega_optimization_unit::ProofQuestionIndexError) -> Self {
        Self::ProofQuestionIndex(error)
    }
}
