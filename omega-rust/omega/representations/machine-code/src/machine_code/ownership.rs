//! Affine cleanup actions and preservation around their emitted suffixes.

use crate::{Aarch64ReturnLinkEvidence, StackAdjustmentPair};
use semantic_vocabulary::{BlockId, EdgeId, OperationId};
use terminal_psi::{
    StructuralPlaceDeclaration, StructuralTypeDeclaration, TerminalAffineCleanupAction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAffineCleanupRecord {
    pub psi_edge: EdgeId,
    /// Canonical verifier-owned type closure retained so object and installed
    /// artifact validation can reconstruct the exact residual partition.
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub locals: Vec<(
        OperationId,
        StructuralPlaceDeclaration,
        StructuralTypeDeclaration,
    )>,
    /// Exact semantic execution order for every cleanup committed by this edge.
    pub actions: Vec<TerminalAffineCleanupAction>,
    pub code_offset: usize,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitContinuationRecord {
    pub operation_ordinal: usize,
    pub successor_operation_ordinal: usize,
    pub source_block: BlockId,
    pub target_block: BlockId,
    /// Exact zero-byte cleanup boundary before the successor's first operation.
    /// Final return cleanup remains a separate record.
    pub cleanup: UnitAffineCleanupRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarControlAffineCleanupRecord {
    pub cleanup: UnitAffineCleanupRecord,
    pub preservation: ScalarCleanupPreservationEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalarCleanupPreservationEvidence {
    pub frame: StackAdjustmentPair,
    pub result_byte_offset: u32,
    pub result_store_offset: usize,
    pub result_load_offset: usize,
    /// AArch64 additionally preserves the caller's link register in the same
    /// lifetime frame. X86-64 uses its implicit stack link and retains `None`.
    pub aarch64_return_link: Option<Aarch64ReturnLinkEvidence>,
}
