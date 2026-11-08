//! calls structural in the legalized operations program.

use abstract_operations::CompletionClaimSource;
use optimization_unit::EffectLink;
use optimization_unit::FuelSettlement;
use optimization_unit::OwnershipEvent;
use semantic_vocabulary::BoundaryMachineId;
use semantic_vocabulary::OperationId;
use semantic_vocabulary::ServiceId;
use target_operations::ClaimCompletionOnlyRealization;
use target_operations::ProviderExecutionBinding;
use terminal_psi::CompletionReceipt;
use terminal_psi::EntryClaim;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPlaceDeclaration;

/// Current structural signature and ownership declarations, without executable rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralContract {
    pub result: Option<terminal_psi::StructuralResultDeclaration>,
    pub structural_types: abstract_operations::StructuralTypeCatalog,
    pub parameters: Vec<LegalizedCallUnitParameter>,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub entry_claims: Vec<EntryClaim>,
    pub published_service_ceiling: Vec<ServiceId>,
}

impl LegalizedStructuralContract {
    /// The structural place keyed `place`, when the contract declares it.
    pub fn structural_place(
        &self,
        place: semantic_vocabulary::PlaceId,
    ) -> Option<&StructuralPlaceDeclaration> {
        self.structural_places.iter().find(|row| row.id == place)
    }

    /// The structural type keyed `id`, when the contract declares it.
    pub fn structural_type(
        &self,
        id: semantic_vocabulary::StructuralTypeId,
    ) -> Option<&terminal_psi::StructuralTypeDeclaration> {
        self.structural_types
            .as_slice()
            .iter()
            .find(|declaration| declaration.id == id)
    }
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
    pub target: target_operations::TargetStructuralParameter,
}

pub use target_operations::NativeCallOrigin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalizedCallSourceError {
    ProviderIdentityMismatch,
    ArgumentSignatureMismatch,
    CompletionEvidenceMismatch,
    OwnershipMismatch,
}
