//! Unresolved internal machine-call fields shared by encoding and layout.

use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFormInternalMachineFixupKind {
    X86Relative32FromNextInstructionToInternalMachineV1,
    Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFormInternalMachineFixupState {
    UnresolvedZeroFieldV1,
}

/// Row-relative unresolved internal-call patch. Layout may translate these
/// coordinates into function-relative custody but may not resolve the patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFormInternalMachineFixup {
    pub kind: SelectedFormInternalMachineFixupKind,
    pub state: SelectedFormInternalMachineFixupState,
    pub callee: MachineId,
    pub opcode_row_offset: u16,
    pub patch_row_offset: u16,
    pub reference_row_offset: u16,
    pub patch_byte_width: u8,
    pub addend: i64,
}
