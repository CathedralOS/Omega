//! Control and relocation obligations of an encoded program. These records
//! contain no source operands or virtual-register definitions. Their control rows
//! describe the encoded blocks whose bytes layout places.
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, MachineSizeKnowledge, SelectedBlockId,
    SelectedImageAddressReference, SelectedInstructionId,
};
use semantic_vocabulary::{BoundaryMachineId, EdgeId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedLayoutRequirements {
    pub selected: selected_instructions::SelectedInstructionPlanIdentity,
    pub machine: std::sync::Arc<post_allocation_machine::PostAllocationMachinePlan>,
    pub target: target::NativeTarget,
    pub functions: Vec<EncodedLayoutFunction>,
    pub alternatives: Vec<EncodedFunctionAlternatives>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedLayoutFunction {
    pub machine: MachineId,
    pub entry_block: SelectedBlockId,
    pub blocks: Vec<EncodedLayoutBlock>,
}
impl EncodedLayoutFunction {
    pub fn block(&self, id: SelectedBlockId) -> Option<&EncodedLayoutBlock> {
        self.blocks.iter().find(|block| block.id == id)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedLayoutBlock {
    pub id: SelectedBlockId,
    pub instructions: Vec<EncodedLayoutRow>,
    pub terminator: EncodedLayoutTerminator,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodedLayoutRow {
    pub id: SelectedInstructionId,
    pub kind: EncodedLayoutOperation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodedLayoutOperation {
    Ordinary,
    CompareI64Zero,
    CopyI64,
    ConditionalBranchNonZero,
    ConditionalBranchU64LessThan,
    ConditionalBranchI64LessThan,
    Call {
        callee: Option<MachineId>,
    },
    NormalizedForeignCall {
        boundary: BoundaryMachineId,
        ordinal: u32,
    },
    ImageAddress {
        reference: SelectedImageAddressReference,
    },
}
impl EncodedLayoutOperation {
    pub fn machine_callee(self) -> Option<MachineId> {
        if let Self::Call { callee } = self {
            callee
        } else {
            None
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodedLayoutSuccessor {
    pub psi_edge: Option<EdgeId>,
    pub block: SelectedBlockId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodedLayoutTerminator {
    Return {
        instruction: EncodedLayoutRow,
    },
    Stop {
        instruction: EncodedLayoutRow,
    },
    Jump {
        instruction: EncodedLayoutRow,
        successor: EncodedLayoutSuccessor,
    },
    ConditionalBranch {
        instruction: EncodedLayoutRow,
        when_nonzero: EncodedLayoutSuccessor,
        when_zero: EncodedLayoutSuccessor,
    },
    ConditionalBranchU64LessThan {
        instruction: EncodedLayoutRow,
        when_less: EncodedLayoutSuccessor,
        when_not_less: EncodedLayoutSuccessor,
    },
    ConditionalBranchI64LessThan {
        instruction: EncodedLayoutRow,
        when_less: EncodedLayoutSuccessor,
        when_not_less: EncodedLayoutSuccessor,
    },
}
impl EncodedLayoutTerminator {
    pub fn instruction(&self) -> &EncodedLayoutRow {
        match self {
            Self::Return { instruction }
            | Self::Stop { instruction }
            | Self::Jump { instruction, .. }
            | Self::ConditionalBranch { instruction, .. }
            | Self::ConditionalBranchU64LessThan { instruction, .. }
            | Self::ConditionalBranchI64LessThan { instruction, .. } => instruction,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFunctionAlternatives {
    pub machine: MachineId,
    pub blocks: Vec<EncodedBlockAlternatives>,
}
impl EncodedFunctionAlternatives {
    pub fn instructions(&self) -> impl Iterator<Item = &EncodedAlternativeRequirement> {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedBlockAlternatives {
    pub block: SelectedBlockId,
    pub instructions: Vec<EncodedAlternativeRequirement>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAlternativeRequirement {
    pub instruction: SelectedInstructionId,
    pub alternative: EncodedAlternativeDeclaration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAlternativeDeclaration {
    pub key: MachineAlternativeKey,
    pub size: MachineSizeKnowledge,
    pub encoded: MachineEncodedEffects,
}
