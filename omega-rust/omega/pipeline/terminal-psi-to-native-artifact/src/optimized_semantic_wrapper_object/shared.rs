pub(crate) use std::collections::BTreeSet;

pub(crate) use isa_x86_64::{
    X86_64_SEMANTIC_UNIT_WRAPPER_CALL_OPCODE_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_FUNCTION_BYTE_COUNT,
    X86_64_SEMANTIC_UNIT_WRAPPER_NEXT_INSTRUCTION_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_OFFSET,
    X86_64_SEMANTIC_UNIT_WRAPPER_REL32_FIELD_WIDTH, X86_64SemanticUnitWrapperResolutionError,
    resolve_x86_64_semantic_unit_wrapper_private_continuation,
};
pub(crate) use object_file::{
    ObjectLocalSymbolId, RelocationFreeObjectPlan, RelocationFreeObjectSymbolRole, SectionKind,
    canonical_private_machine_symbol_name, section_name,
};
pub(crate) use object_file::{
    OptimizedObjectArtifactError, StagedValidatedOptimizedObjectArtifact,
    validate_optimized_object_artifact,
};
pub(crate) use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
pub(crate) use program_entry_plan::{
    OptimizedProgramStorageSemanticEntryContract,
    bind_optimized_program_storage_semantic_entry_contract,
    plan_optimized_program_storage_semantic_wrapper,
};
pub(crate) use selected_instructions::{SelectedInstructionPlan, SelectedStructuralUnitCallSource};
pub(crate) use semantic_vocabulary::{IntegerSign, MachineId, ScalarType, StructuralPlaceKind};
pub(crate) use target::{NativeTarget, ObjectFormat};
pub(crate) use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralTypeShape, TerminalMachineResult, TerminalPsiIdentity,
    VocabularyMarker,
};
pub(crate) use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) use crate::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    OptimizedProgramStorageSemanticWrapperEncodingError,
    StagedOptimizedProgramStorageSemanticWrapperEncoding, ValidatedNativeProgramEntrySettlement,
    validate_native_program_entry_settlement,
    validate_optimized_program_storage_semantic_wrapper_encoding,
};

pub(crate) const PLAN_SCHEMA: &[u8] =
    b"omega.optimized-program-storage-semantic-wrapper-object.v1\0";
pub(crate) const CONTAINER_MAGIC: &[u8; 8] = b"OMGPSO\0\0";
pub(crate) const MANIFEST_MAGIC: &[u8; 8] = b"OMGPSM\0\0";
pub(crate) const CODEC_VERSION: u32 = 1;
pub(crate) const WRAPPER_SYMBOL_NAME: &str = "__omega_program_entry_plan_semantic_wrapper_v1";
