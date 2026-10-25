use super::receipt;
use super::{MachineEffectError, ValidatedPreAllocationMachineEffects};
use selected_instructions::PreAllocationMachineEffectPlan;

#[allow(clippy::too_many_arguments)]
/// Seal effects computed in this compile from these exact inputs.
pub(crate) fn seal_pre_allocation_machine_effects(
    plan: PreAllocationMachineEffectPlan,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
    let receipt = receipt(&plan);
    Ok(ValidatedPreAllocationMachineEffects::new(plan, receipt))
}
