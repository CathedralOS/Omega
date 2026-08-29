use super::prelude::*;
use super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationStatistics {
    pub functions: u64,
    pub blocks: u64,
    pub instructions: u64,
    pub bytes: u64,
    pub resolved_conditional_branches: u64,
    pub structural_unit_functions: u64,
    pub structural_unit_blocks: u64,
    pub structural_unit_instructions: u64,
    pub structural_unit_bytes: u64,
    pub unresolved_internal_machine_fixups: u64,
}

/// Structured report at the function-relative selected-form boundary after
/// validating the admitted whole-function frameless exit discipline. It owns
/// no frame, section, symbol, relocation, executable image, installation, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationManifest {
    pub identity: FunctionRelativeOptimizationRealizationManifestIdentity,
    pub stage: FunctionRelativeOptimizationRealizationStage,
    pub selections: OptimizationSelectionIdentity,
    pub selected_lowering_selections: OptimizationSelectionIdentity,
    pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pub allocation_recovery_selections: OptimizationSelectionIdentity,
    pub post_allocation_machine_selections: OptimizationSelectionIdentity,
    pub function_relative_layout_selections: OptimizationSelectionIdentity,
    pub pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub pre_allocation_machine_effects: omega_machine_optimizer::PreAllocationMachineEffectIdentity,
    pub post_allocation_machine: omega_machine_optimizer::PostAllocationMachineIdentity,
    pub baseline_pre_layout: SelectedFormEncodingIdentity,
    pub pre_layout: SelectedFormEncodingIdentity,
    pub baseline_resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub resolved_layout: ResolvedSelectedFormLayoutIdentity,
    pub x86_branch_relaxation: Option<X86BranchRelaxationIdentity>,
    pub post_allocation_machine_optimization:
        Option<crate::PostAllocationMachineOptimizationCustody>,
    pub whole_function_exit_contract: WholeFunctionExitContractIdentity,
    pub target: NativeTarget,
    pub layout_policy: SelectedFunctionLayoutPolicy,
    pub scope: FunctionRelativeOptimizationRealizationScope,
    pub statistics: FunctionRelativeOptimizationRealizationStatistics,
    pub frame: FunctionRelativeOptimizationUnavailableData,
    pub machine_emission: FunctionRelativeOptimizationUnavailableData,
    pub section_placement: FunctionRelativeOptimizationUnavailableData,
    pub symbols: FunctionRelativeOptimizationUnavailableData,
    pub object_relocations: FunctionRelativeOptimizationUnavailableData,
    pub executable_image: FunctionRelativeOptimizationUnavailableData,
    pub installation: FunctionRelativeOptimizationUnavailableData,
    pub publication: FunctionRelativeOptimizationUnavailableData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionRelativeOptimizationRealizationManifest {
    pub(super) record: FunctionRelativeOptimizationRealizationManifest,
}

impl ValidatedFunctionRelativeOptimizationRealizationManifest {
    pub const fn record(&self) -> &FunctionRelativeOptimizationRealizationManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut FunctionRelativeOptimizationRealizationManifest {
        &mut self.record
    }
}
