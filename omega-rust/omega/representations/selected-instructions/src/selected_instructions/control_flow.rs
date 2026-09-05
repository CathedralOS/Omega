//! Functions, blocks and explicitly identified semantic successor edges.
use super::{SelectedBlockId, SelectedInstruction, VirtualRegister};
use abstract_operations::ValueBinding;
use optimization_unit::FuelSettlement;
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFunction {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub entry_block: SelectedBlockId,
    pub virtual_registers: Vec<VirtualRegister>,
    pub blocks: Vec<SelectedBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBlock {
    pub id: SelectedBlockId,
    pub source_block: BlockId,
    pub instructions: Vec<SelectedInstruction>,
    pub terminator: SelectedTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTerminator {
    ConditionalBranch {
        instruction: SelectedInstruction,
        when_nonzero: SelectedSuccessor,
        when_zero: SelectedSuccessor,
    },
    /// Predicate-aware unsigned-U64 control. Keeping the semantic successors
    /// distinct prevents later persistence and allocation stages from having
    /// to infer ordering meaning from a generic nonzero/zero branch.
    ConditionalBranchU64LessThan {
        instruction: SelectedInstruction,
        when_less: SelectedSuccessor,
        when_not_less: SelectedSuccessor,
    },
    /// Predicate-aware signed-I64 strict-less-than control.
    ConditionalBranchI64LessThan {
        instruction: SelectedInstruction,
        when_less: SelectedSuccessor,
        when_not_less: SelectedSuccessor,
    },
    Return {
        instruction: SelectedInstruction,
        psi_return_edge: EdgeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSuccessor {
    pub psi_edge: EdgeId,
    pub block: SelectedBlockId,
    pub source_target: BlockId,
    pub bindings: Vec<ValueBinding>,
    /// Path-specific logical fuel for this exact semantic edge.
    pub fuel: Vec<FuelSettlement>,
}
