// Where a row of a native program form came from and what it is charged:
// its Psi operation or edge, its logical work, its place in effect order, and
// the ownership events it carries. Every form from the optimization unit to
// selected instructions keeps these unchanged, so they have one owner.

use semantic_vocabulary::{BlockId, ClaimId, EdgeId, OperationId, ScalarType, ValueId};
use terminal_psi::TerminalAffineCleanupAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsiProvenance {
    Operation(OperationId),
    Edge(EdgeId),
}

impl From<OperationId> for PsiProvenance {
    fn from(operation: OperationId) -> Self {
        Self::Operation(operation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkSettlement {
    pub site: PsiProvenance,
    pub units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectLink {
    pub input: u64,
    pub output: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueDefinitionSite {
    FunctionParameter(u32),
    BlockParameter { block: BlockId, position: u32 },
    Node { block: BlockId, node: u32 },
}

pub fn encode_value_definition_site_identity(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            optimization_core::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            optimization_core::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            optimization_core::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            optimization_core::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            optimization_core::codec_bytes::push_encoded(bytes, node.to_le_bytes());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueDefinition {
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub site: ValueDefinitionSite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipEvent {
    ClaimTransfer(Vec<ClaimId>),
    ClaimCompletion(Vec<terminal_psi::CompletionReceipt>),
    Cleanup(Vec<TerminalAffineCleanupAction>),
    StructuralReturn(Vec<ClaimId>),
    CrashFrontier(Vec<ClaimId>),
    ExternalCompletionFrontier(Vec<ClaimId>),
}
