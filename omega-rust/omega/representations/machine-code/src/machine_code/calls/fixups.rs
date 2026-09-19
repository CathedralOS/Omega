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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFormNormalizedForeignCallFixupKind {
    X86Relative32FromNextInstructionToNormalizedForeignImportV1,
    Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFormNormalizedForeignCallFixupState {
    /// The patch field stays zero through placement; object construction binds
    /// it to the declared normalized import symbol.
    UnresolvedImportFieldV1,
}

/// Row-relative unresolved normalized-foreign-call patch. The `{boundary,
/// ordinal}` pair names the exact selected roster row owning the evaluated
/// locator; the raw foreign address never appears here. Placement may
/// translate these coordinates into section-relative custody but must leave
/// the field unresolved for object construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFormNormalizedForeignCallFixup {
    pub kind: SelectedFormNormalizedForeignCallFixupKind,
    pub state: SelectedFormNormalizedForeignCallFixupState,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_row_offset: u16,
    pub patch_row_offset: u16,
    pub reference_row_offset: u16,
    pub patch_byte_width: u8,
    pub addend: i64,
}
