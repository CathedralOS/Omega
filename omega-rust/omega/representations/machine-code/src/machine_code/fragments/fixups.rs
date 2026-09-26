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

/// Function-relative unresolved normalized-foreign-call field. The
/// `{boundary, ordinal}` pair names the selected roster row owning the
/// evaluated locator; no raw foreign coordinate is stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
