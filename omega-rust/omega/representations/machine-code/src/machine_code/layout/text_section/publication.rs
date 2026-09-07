//! Text-section publication claims and their canonical representation.
//!
//! Decoding establishes canonical data, not source, placement, or publication admission.
mod codec;
mod error;
#[cfg(test)]
mod tests;
pub use error::FunctionFragmentTextSectionManifestDecodeError;

use super::{TextSectionPlacementPolicy, TextSectionRelocationRequirements};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

use crate::{
    FunctionFragmentFrameApplicationIdentity, ResolvedSelectedFormLayoutIdentity,
    SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionStage {
    ValidatedFixedFrameInternalCallTextSectionPlacementV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub padding_bytes: u64,
    pub relocation_requirements: u64,
    pub source_internal_machine_fixups: u64,
    pub resolved_internal_machine_fixups: u64,
    pub remaining_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionManifest {
    pub identity: FunctionFragmentTextSectionManifestIdentity,
    pub stage: FunctionFragmentTextSectionStage,
    pub frame_application: FunctionFragmentFrameApplicationIdentity,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: physical_instructions::PostAllocationMachineIdentity,
    pub final_pre_layout: SelectedFormEncodingIdentity,
    pub final_resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub fragments: FunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_offset: u64,
    pub placement_policy: TextSectionPlacementPolicy,
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    pub relocation_requirements: TextSectionRelocationRequirements,
    pub statistics: FunctionFragmentTextSectionStatistics,
    pub symbols: FunctionFragmentTextSectionUnavailableData,
    pub object_container: FunctionFragmentTextSectionUnavailableData,
    pub external_entry_bridge: FunctionFragmentTextSectionUnavailableData,
    pub executable_image: FunctionFragmentTextSectionUnavailableData,
    pub installation: FunctionFragmentTextSectionUnavailableData,
    pub publication: FunctionFragmentTextSectionUnavailableData,
}
