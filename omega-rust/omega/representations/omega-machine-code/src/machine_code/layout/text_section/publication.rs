//! Text-section publication claims and their canonical representation.
//!
//! Decoding establishes canonical data, not source, placement, or publication admission.
mod codec;
mod error;
#[cfg(test)]
mod tests;
pub use error::FunctionFragmentTextSectionManifestDecodeError;

use super::{TextSectionPlacementPolicy, TextSectionRelocationRequirements};
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationSelectionIdentity,
    PostAllocationOptimizationManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    FunctionFragmentEmissionSourceKind, FunctionFragmentFrameApplicationIdentity,
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity,
    WholeFunctionExitContractIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionStage {
    ValidatedRelocationFreeTextSectionPlacementV1,
    ValidatedFixedFrameInternalCallTextSectionPlacementV1,
}

/// Role-specific custody for the fragment representation consumed by text
/// placement. The frame-applied role binds the exact application that shifted
/// instruction and fixup coordinates; it is not inferred from final bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionSourceCustody {
    DirectFragmentEmissionV1,
    FixedFrameApplicationV1 {
        application: FunctionFragmentFrameApplicationIdentity,
    },
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
    pub structural_unit_functions: u64,
    pub structural_unit_blocks: u64,
    pub structural_unit_instruction_spans: u64,
    pub structural_unit_zero_byte_instruction_spans: u64,
    pub structural_unit_bytes: u64,
    pub source_internal_machine_fixups: u64,
    pub resolved_internal_machine_fixups: u64,
    pub remaining_internal_machine_fixups: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentTextSectionManifest {
    pub identity: FunctionFragmentTextSectionManifestIdentity,
    pub stage: FunctionFragmentTextSectionStage,
    pub source_custody: FunctionFragmentTextSectionSourceCustody,
    pub source_kind: FunctionFragmentEmissionSourceKind,
    pub source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_physical_instructions::PostAllocationMachineIdentity,
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
