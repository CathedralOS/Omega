pub use super::block::Block;
use crate::{
    ContentIdentityReshuffle, ContentPartitionComposition, EntryClaim, MachineContract, Operation,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalRankedScc,
    ValueDeclaration,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, StructuralTypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMachine {
    /// Formation facts for calls whose source markers must survive lowering.
    /// Sorted by site; these do not select a callee or create a pause.
    pub call_formations: Vec<crate::CallFormation>,
    pub assembly_target_requirement: crate::RegisterTargetApplicability,
    pub id: MachineId,
    pub attachment: Option<StructuralTypeId>,
    pub ranked_scc: Option<TerminalRankedScc>,
    pub structural_places: Vec<StructuralPlaceDeclaration>,
    pub block_entry_claims: Vec<EntryClaim>,
    pub loan_completions: Vec<crate::LoanCompletion>,
    pub content_identity_reshuffles: Vec<ContentIdentityReshuffle>,
    pub content_partition_compositions: Vec<ContentPartitionComposition>,
    pub entry: BlockId,
    pub blocks: Vec<Block>,
    pub contract: MachineContract,
}

// Existing machine-header readers borrow the same declaration contract.
// This view stores nothing and cannot give a bodyless contract executable code.
impl std::ops::Deref for TerminalMachine {
    type Target = MachineContract;

    fn deref(&self) -> &Self::Target {
        &self.contract
    }
}

impl std::ops::DerefMut for TerminalMachine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contract
    }
}

impl TerminalMachine {
    pub fn entry_proposition_context(
        &self,
    ) -> Result<semantic_vocabulary::PropositionContext, semantic_vocabulary::PropositionError>
    {
        semantic_vocabulary::PropositionContext::from_value_types_and_places(
            self.parameters
                .iter()
                .chain(&self.contract.erased_scalar_formals)
                .map(|value| (value.id, value.scalar_type)),
            self.structural_places
                .iter()
                .filter(|place| {
                    matches!(
                        place.kind,
                        semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
                    )
                })
                .map(|place| (place.id, place.kind)),
        )
    }
    pub fn structural_place_type(&self, place: PlaceId) -> Option<StructuralTypeId> {
        self.structural_parameter(place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                self.structural_places.iter().find_map(|row| {
                    if row.id != place {
                        return None;
                    }
                    match row.kind {
                        semantic_vocabulary::StructuralPlaceKind::Result => self
                            .result
                            .structural()
                            .filter(|result| result.place == place)
                            .map(|result| result.structural_type),
                        _ => row.kind.produced_structural_type(),
                    }
                })
            })
    }

    pub fn block(&self, id: &BlockId) -> Option<&Block> {
        // The interpreter looks up a block on every step. A checked machine
        // stores block `n` at position `n - 1`, so that slot answers directly;
        // a machine that is not yet checked falls back to the scan.
        usize::try_from(id.get() - 1)
            .ok()
            .and_then(|position| self.blocks.get(position))
            .filter(|block| block.id == *id)
            .or_else(|| self.blocks.iter().find(|block| block.id == *id))
    }

    pub fn operations(&self) -> impl Iterator<Item = &Operation> + '_ {
        self.blocks.iter().flat_map(|block| &block.operations)
    }

    pub fn block_parameters(&self) -> impl Iterator<Item = &ValueDeclaration> + '_ {
        self.blocks.iter().flat_map(|block| &block.parameters)
    }

    pub fn block_structural_parameters(
        &self,
    ) -> impl Iterator<Item = &StructuralParameterDeclaration> + '_ {
        self.blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
    }

    pub fn operation(&self, id: OperationId) -> Option<&Operation> {
        self.operations().find(|operation| operation.id == id)
    }

    pub fn structural_place(&self, place: PlaceId) -> Option<&StructuralPlaceDeclaration> {
        self.structural_places
            .iter()
            .find(|entry| entry.id == place)
    }

    pub fn producing_operation(&self, place: PlaceId) -> Option<(&Operation, StructuralTypeId)> {
        let declaration = self.structural_place(place)?;
        let (producer, structural_type) = declaration.kind.as_operation_result()?;
        let operation = self
            .operations()
            .find(|operation| operation.id == producer)?;
        Some((operation, structural_type))
    }

    pub fn structural_parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .chain(self.block_structural_parameters())
            .find(|parameter| parameter.place == place)
    }
    pub fn parameter(&self, place: PlaceId) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameter(place)
    }
    pub fn signature_structural_parameter(
        &self,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        self.structural_parameters
            .iter()
            .find(|parameter| parameter.place == place)
    }
}

pub fn structural_parameter_position(
    parameters: &[StructuralParameterDeclaration],
    place: PlaceId,
) -> Option<usize> {
    parameters
        .iter()
        .position(|parameter| parameter.place == place)
}
