//! calls structural in the legalized operations program.

use abstract_operations_to_target_operations::target_operations::ClaimCompletionOnlyRealization;
use abstract_operations_to_target_operations::target_operations::ProviderExecutionBinding;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::ServiceId;
use terminal_psi::CompletionReceipt;
use terminal_psi::EntryClaim;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPlaceDeclaration;
use terminal_psi_to_abstract_operations::abstract_operations::CompletionClaimSource;
use terminal_psi_to_abstract_operations::optimization_unit::EffectLink;
use terminal_psi_to_abstract_operations::optimization_unit::FuelSettlement;
use terminal_psi_to_abstract_operations::optimization_unit::OwnershipEvent;

/// Current structural signature and ownership declarations, without executable rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralContract {
    pub result: Option<terminal_psi::StructuralResultDeclaration>,
    pub structural_types:
        terminal_psi_to_abstract_operations::abstract_operations::StructuralTypeCatalog,
    pub parameters: Vec<LegalizedCallUnitParameter>,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<EntryClaim>,
    pub published_service_ceiling: Vec<ServiceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedBoundarySettlement {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub provider_execution: ProviderExecutionBinding,
    pub realization: ClaimCompletionOnlyRealization,
    pub arguments: Vec<StructuralArgument>,
    pub completion_claim_sources: Vec<CompletionClaimSource>,
    pub completion_receipts: Vec<CompletionReceipt>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
    pub ownership: Vec<OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedCallUnitParameter {
    pub semantic: StructuralParameterDeclaration,
    pub target:
        abstract_operations_to_target_operations::target_operations::TargetStructuralParameter,
}

pub use abstract_operations_to_target_operations::target_operations::NativeCallOrigin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedCallSourceError {
    ProviderIdentityMismatch,
    ArgumentSignatureMismatch,
    CompletionEvidenceMismatch,
    OwnershipMismatch,
}
