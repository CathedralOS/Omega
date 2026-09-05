//! Function-fragment publication records and their canonical representation.
//!
//! Decoding and recomputing an identity check representation consistency only.
//! Stage labels are retained claims; neither they nor this record grant admission.

mod codec;
mod error;
#[cfg(test)]
mod tests;
pub use error::FunctionFragmentEmissionManifestDecodeError;

use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationSelectionIdentity, PostAllocationOptimizationManifestIdentity,
};
use omega_target::NativeTarget;
use psi_core::FuelScheduleIdentity;
use psi_terminal::TerminalPsiIdentity;

use crate::{SelectedFormEncodingIdentity, WholeFunctionExitContractIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionSourceKind {
    X86Rel8V1,
    SelectedLoweringV1,
    PostAllocationMachineOptimizationV1 { optimization: Optimization },
    AllocationRecoveryV1,
    UnitBaselineV1,
    StructuralUnitV1,
    CanonicalFixedFrameBodyV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionStage {
    ValidatedRelocationFreeFunctionFragmentsV1,
    ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionFragmentEmissionStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instruction_spans: u64,
    pub zero_byte_instruction_spans: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub logical_fuel_settlements: u64,
    pub structural_unit_functions: u64,
    pub structural_unit_blocks: u64,
    pub structural_unit_instruction_spans: u64,
    pub structural_unit_bytes: u64,
    pub unresolved_internal_machine_fixups: u64,
    pub structural_logical_fuel_settlements: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentEmissionManifest {
    pub identity: FunctionFragmentEmissionManifestIdentity,
    pub stage: FunctionFragmentEmissionStage,
    pub source_kind: FunctionFragmentEmissionSourceKind,
    pub source_realization: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub post_allocation_machine: omega_physical_instructions::PostAllocationMachineIdentity,
    pub final_pre_layout: SelectedFormEncodingIdentity,
    pub final_resolved_layout: crate::ResolvedSelectedFormLayoutIdentity,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub fragments: FunctionFragmentEmissionIdentity,
    pub target: NativeTarget,
    pub statistics: FunctionFragmentEmissionStatistics,
    pub section_placement: FunctionFragmentEmissionUnavailableData,
    pub symbols: FunctionFragmentEmissionUnavailableData,
    pub object_relocations: FunctionFragmentEmissionUnavailableData,
    pub executable_image: FunctionFragmentEmissionUnavailableData,
    pub installation: FunctionFragmentEmissionUnavailableData,
    pub publication: FunctionFragmentEmissionUnavailableData,
}
