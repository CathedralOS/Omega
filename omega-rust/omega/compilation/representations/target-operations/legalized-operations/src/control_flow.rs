//! Prepared scalar control graphs use shared arithmetic and operand records.

#[path = "control_flow_scalar_graph.rs"]
mod scalar_graph;
pub use scalar_graph::{
    LegalizedCasePointerSlot, LegalizedDescriptorOrigin, LegalizedNormalizedForeignCall,
    LegalizedRuntimeIndexOperand, LegalizedScalarArgument, LegalizedScalarBlock,
    LegalizedScalarCall, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedStructuralBinding, LegalizedStructuralCaseSource,
    LegalizedValueDefinition,
};

use native_vocabulary::provenance::WorkSettlement;
use semantic_vocabulary::{BlockId, EdgeId, StructuralCaseId};

pub use target_vocabulary::{
    LegalizedStructuralCasePayload, SaturatingCarrier, SaturatingOperation, TrappingForm,
    TrappingOperation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralCaseSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<LegalizedStructuralCasePayload>,
    pub structural_bindings: Vec<LegalizedStructuralBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
    pub cleanup_calls: Vec<(target_vocabulary::CallSiteOwner, LegalizedScalarCall)>,
    pub work: Vec<WorkSettlement>,
}
