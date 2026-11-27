use crate::{
    ErasedProofFormal, Operation, StructuralParameterDeclaration, Terminator, ValueDeclaration,
};
use semantic_vocabulary::BlockId;

/// A physical requirement on this block's ordinary Psi exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyExitConstraint {
    /// No instruction or register setup may implement this edge.
    Adjacent {
        target: BlockId,
        /// Each register holds the scalar argument at the same position on this edge.
        registers: Vec<crate::AsmRegisterView>,
    },
    /// At return the named register view holds exactly the returned SSA value.
    ScalarReturn(crate::AsmRegisterView),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub parameters: Vec<ValueDeclaration>,
    pub erased_scalar_formals: Vec<ValueDeclaration>,
    pub erased_proof_formals: Vec<ErasedProofFormal>,
    pub structural_parameters: Vec<StructuralParameterDeclaration>,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
    pub assembly_exit: Option<AssemblyExitConstraint>,
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
            assembly_exit: None,
        }
    }
}
