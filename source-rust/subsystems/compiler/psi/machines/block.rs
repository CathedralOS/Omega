use crate::{
    ErasedProofFormal, Operation, StructuralParameterDeclaration, Terminator, ValueDeclaration,
};
use semantic_vocabulary::BlockId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    pub erased_scalar_formals: Vec<ValueDeclaration>,
    pub erased_proof_formals: Vec<ErasedProofFormal>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}

impl Block {
    pub fn plain(id: BlockId, parameters: Vec<ValueDeclaration>, terminator: Terminator) -> Self {
        Self {
            id,
            parameters,
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator,
        }
    }
}
