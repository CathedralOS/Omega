//! Structural-call spans with unresolved section-dependent internal fixups.

use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId};

use super::ResolvedSelectedFormRow;
use crate::{X86_64SelectedStructuralUnitCallFootprint, X86_64StructuralUnitInternalControlFixup};

/// Function-relative custody for the canonical structural Unit call template.
/// The bytes deliberately retain their zero rel32 placeholder; `fixup` remains
/// unresolved until whole-text placement knows both MachineId coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStructuralUnitCallLayout {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub footprint: Box<X86_64SelectedStructuralUnitCallFootprint>,
    pub fixup: X86_64StructuralUnitInternalControlFixup,
}

/// Exact one-block function-relative span for the bounded structural Unit
/// route. A caller is 89 unresolved call bytes plus one `C3`; a leaf is the
/// single `C3` byte. This carrier grants neither section placement nor
/// executable-byte authority while `call.fixup` remains unresolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStructuralUnitFunctionLayout {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub call: Option<ResolvedStructuralUnitCallLayout>,
    pub return_instruction: ResolvedSelectedFormRow,
}
