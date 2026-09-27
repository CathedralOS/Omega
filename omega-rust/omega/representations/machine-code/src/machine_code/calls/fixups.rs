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

/// How one callback-address materialization names its unresolved field(s).
/// Offsets are relative to the containing selected-form row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedFormCallbackMaterializationEncoding {
    /// `lea reg, [rip + rel32]`; the disp32 is the private-symbol field.
    X86_64Relative32 { patch_row_offset: u16 },
    /// `adrp` + `add #lo12`; both immediate fields are private-symbol fields.
    Aarch64PageAddress {
        page_patch_row_offset: u16,
        page_offset_patch_row_offset: u16,
    },
}

/// The private-function-address materialization emitted inside one callback-
/// bearing normalized foreign call row, ahead of the call bytes. Its fields
/// stay unresolved through placement; object construction binds them to the
/// thunk's private symbol, never to an import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedFormCallbackMaterialization {
    /// Where the materialized address lands — the exact register or outgoing
    /// stack slot the evaluated callback application names.
    pub destination: crate::CallbackAddressDestination,
    /// Row offset where the materialization bytes begin.
    pub code_row_offset: u16,
    /// Total bytes the materialization occupies within the row.
    pub code_byte_count: u16,
    pub encoding: SelectedFormCallbackMaterializationEncoding,
}

/// Row-relative unresolved normalized-foreign-call patch. The `{boundary,
/// ordinal}` pair names the exact selected roster row owning the evaluated
/// locator; the raw foreign address never appears here. Placement may
/// translate these coordinates into section-relative custody but must leave
/// the field unresolved for object construction.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The private callback materializations emitted inside the same row's
    /// bytes, in evaluated roster order — one entry per callback
    /// destination the call's private schedule names.
    pub callbacks: Vec<SelectedFormCallbackMaterialization>,
}
