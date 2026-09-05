//! Object-format publication claims, separate from source admission and custody.
mod codec;
mod statistics;
#[cfg(test)]
mod tests;
use crate::{
    ObjectLocalSymbolId, RelocationFreeObjectRelocationRequirements,
    RelocationFreeObjectSymbolPolicy,
};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
pub use statistics::relocation_free_object_statistics;
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerStage {
    ValidatedRelocationFreeObjectContainerV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerStatistics {
    pub sections: u64,
    pub function_symbols: u64,
    pub object_local_symbols: u64,
    pub external_symbols: u64,
    pub text_bytes: u64,
    pub container_bytes: u64,
    pub relocation_records: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentObjectContainerManifest {
    pub identity: FunctionFragmentObjectContainerManifestIdentity,
    pub stage: FunctionFragmentObjectContainerStage,
    pub source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub symbol_policy: RelocationFreeObjectSymbolPolicy,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub relocation_requirements: RelocationFreeObjectRelocationRequirements,
    pub statistics: FunctionFragmentObjectContainerStatistics,
    pub external_entry_bridge: FunctionFragmentObjectContainerUnavailableData,
    pub executable_image: FunctionFragmentObjectContainerUnavailableData,
    pub installation: FunctionFragmentObjectContainerUnavailableData,
    pub publication: FunctionFragmentObjectContainerUnavailableData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentObjectContainerManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownVocabulary(u16),
    InvalidFuelSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    InvalidSemanticEntry,
    InvalidSymbolId,
    UnknownSymbolPolicy,
    UnknownRelocationRequirements,
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}
