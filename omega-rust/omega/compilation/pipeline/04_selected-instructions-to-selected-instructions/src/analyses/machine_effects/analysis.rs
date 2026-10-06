use crate::analyses::analyze_pre_allocation_machine_effects;
use crate::ValidatedPreAllocationMachineEffects;

use register_environment::ValidatedTargetRegisterEnvironment;

use super::MachineEffectStageError;
use super::catalog::validated_catalog;

pub(super) fn analyze<S: crate::ValidatedSelectedAnalysis>(
    selected: &S,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectStageError> {
    let catalog = validated_catalog(selected.selected_plan().target, environment.constraints())?;
    analyze_pre_allocation_machine_effects(
        selected,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        &catalog,
    )
    .map_err(MachineEffectStageError::Analysis)
}

