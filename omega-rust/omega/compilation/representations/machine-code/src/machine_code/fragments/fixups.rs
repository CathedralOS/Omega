//! Typed internal call fields awaiting whole-text placement.

use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentInternalMachineFixupKind {
    X86Relative32FromNextInstructionToInternalMachineV1,
    Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentInternalMachineFixupState {
    UnresolvedZeroFieldV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentInternalMachineFixup {
    pub kind: FunctionFragmentInternalMachineFixupKind,
    pub state: FunctionFragmentInternalMachineFixupState,
    pub callee: MachineId,
    pub opcode_function_offset: u64,
    /// Start of the encoded storage region changed by resolution.
    pub patch_function_offset: u64,
    /// Architecture-defined PC-relative reference coordinate.
    pub reference_function_offset: u64,
    pub patch_byte_width: u8,
    pub addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentNormalizedForeignCallFixupKind {
    X86Relative32FromNextInstructionToNormalizedForeignImportV1,
    Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentNormalizedForeignCallFixupState {
    /// The patch field stays zero; object construction binds it to the
    /// declared normalized import symbol.
    UnresolvedImportFieldV1,
}

/// How one fragment-level callback-address materialization names its
/// unresolved field(s). Offsets are relative to the containing function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentCallbackMaterializationEncoding {
    /// `lea reg, [rip + rel32]`; the disp32 is the private-symbol field.
    X86_64Relative32 { patch_function_offset: u64 },
    /// `adrp` + `add #lo12`; both immediate fields are private-symbol fields.
    Aarch64PageAddress {
        page_patch_function_offset: u64,
        page_offset_patch_function_offset: u64,
    },
}

/// Function-relative evidence for the private-function-address
/// materialization emitted inside one callback-bearing normalized foreign
/// call. Its fields stay unresolved for object construction, which binds them
/// to the thunk's private symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentCallbackMaterialization {
    /// Where the materialized address lands — the exact register or outgoing
    /// stack slot the evaluated callback application names.
    pub destination: crate::CallbackAddressDestination,
    /// Function offset where the materialization bytes begin.
    pub code_function_offset: u64,
    /// Total bytes the materialization occupies.
    pub code_byte_count: u64,
    pub encoding: FunctionFragmentCallbackMaterializationEncoding,
}

/// Function-relative unresolved normalized-foreign-call field. The
/// `{boundary, ordinal}` pair names the selected roster row owning the
/// evaluated locator; no raw foreign coordinate is stored here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentNormalizedForeignCallFixup {
    pub kind: FunctionFragmentNormalizedForeignCallFixupKind,
    pub state: FunctionFragmentNormalizedForeignCallFixupState,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_function_offset: u64,
    /// Start of the encoded storage region the import binding writes.
    pub patch_function_offset: u64,
    /// Architecture-defined PC-relative reference coordinate.
    pub reference_function_offset: u64,
    pub patch_byte_width: u8,
    pub addend: i64,
    /// The private callback materializations emitted inside the same
    /// instruction's bytes, in evaluated roster order — one entry per
    /// callback destination the call's private schedule names.
    pub callbacks: Vec<FunctionFragmentCallbackMaterialization>,
}
