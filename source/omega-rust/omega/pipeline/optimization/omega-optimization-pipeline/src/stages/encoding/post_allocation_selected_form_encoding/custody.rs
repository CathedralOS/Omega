use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_target::Architecture;

use crate::{
    PostAllocationMachineOptimizationCustody, StagedOptimizedPostAllocationMachineOptimization,
    StagedOptimizedPostAllocationMachinePlan,
};

use super::OptimizedSelectedFormEncodingError;

pub(super) fn validate_optimization_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
) -> Result<PostAllocationMachineOptimizationCustody, OptimizedSelectedFormEncodingError> {
    let normalized = optimization
        .custody()
        .ok_or(OptimizedSelectedFormEncodingError::ArtifactMismatch)?;
    let roots_match = match optimization {
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Cbnz(fusion) => {
            let receipt = fusion.fusion().receipt();
            let plan = fusion.fusion().plan();
            selected.selected_plan().target.architecture == Architecture::Aarch64
                && receipt.selected() == selected.selected_identity()
                && receipt.source() == machine.machine().receipt().identity()
                && receipt.identity() == fusion.custody().fusion()
                && receipt.action_count() == fusion.custody().action_count()
                && plan.target == selected.selected_plan().target
                && plan.physical_register_model == physical.identity()
        }
        StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) => {
            let receipt = materialization.materialization().receipt();
            let plan = materialization.materialization().plan();
            selected.selected_plan().target.architecture == Architecture::Aarch64
                && receipt.selected() == selected.selected_identity()
                && receipt.source() == machine.machine().receipt().identity()
                && receipt.identity() == materialization.custody().materialization()
                && receipt.action_count() == materialization.custody().action_count()
                && receipt.baseline_words() == materialization.custody().baseline_words()
                && receipt.selected_words() == materialization.custody().selected_words()
                && plan.target == selected.selected_plan().target
                && plan.physical_register_model == physical.identity()
        }
        StagedOptimizedPostAllocationMachineOptimization::X86XorZero(materialization) => {
            let receipt = materialization.materialization().receipt();
            let plan = materialization.materialization().plan();
            selected.selected_plan().target.architecture == Architecture::X86_64
                && receipt.selected() == selected.selected_identity()
                && receipt.source() == machine.machine().receipt().identity()
                && receipt.identity() == materialization.custody().materialization()
                && receipt.action_count() == materialization.custody().action_count()
                && receipt.baseline_bytes() == materialization.custody().baseline_bytes()
                && receipt.selected_bytes() == materialization.custody().selected_bytes()
                && plan.target == selected.selected_plan().target
                && plan.physical_register_model == physical.identity()
        }
        StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(materialization) => {
            let receipt = materialization.materialization().receipt();
            let plan = materialization.materialization().plan();
            selected.selected_plan().target.architecture == Architecture::X86_64
                && receipt.selected() == selected.selected_identity()
                && receipt.source() == machine.machine().receipt().identity()
                && receipt.identity() == materialization.custody().materialization()
                && receipt.action_count() == materialization.custody().action_count()
                && receipt.baseline_bytes() == materialization.custody().baseline_bytes()
                && receipt.selected_bytes() == materialization.custody().selected_bytes()
                && plan.target == selected.selected_plan().target
                && plan.physical_register_model == physical.identity()
        }
    };
    if !roots_match
        || normalized.source() != machine.machine().receipt().identity()
        || normalized.selections() != optimization.selections()
        || normalized.action_count() != optimization.action_count()
    {
        return Err(OptimizedSelectedFormEncodingError::SelectedRootMismatch);
    }
    Ok(normalized)
}
