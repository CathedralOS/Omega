//! Raw selected and post-allocation validation helpers.

use crate::tests::*;

pub(crate) fn validate_raw_selection(
    staged: &StagedOptimizedSelectedInstructions,
    raw: selected_instructions::SelectedInstructionPlan,
) -> Result<
    target_operations_to_selected_instructions::ValidatedSelectedInstructions,
    SelectedInstructionError,
> {
    let constraints = target_operations_to_selected_instructions::selection_constraints(
        staged.legalized(),
        staged.register_environment(),
    );
    validate_selected_instructions(
        staged.legalized(),
        &constraints,
        staged.register_environment().physical(),
        staged.register_environment().constraints(),
        raw,
    )
}

pub(crate) fn validate_raw_post_allocation(
    source: &StagedOptimizedRegisterHomes,
    staged: &StagedOptimizedPostAllocationMachinePlan,
    raw: physical_instructions::PostAllocationMachinePlan,
) -> Result<
    register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan,
    register_homes_to_post_allocation_machine::PostAllocationMachineError,
> {
    let selected = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected.register_environment();
    register_homes_to_post_allocation_machine::validate_post_allocation_machine_plan(
        selected.selected(),
        staged.effects(),
        source.legality_stage().live_range_stage().ranges(),
        source.legality_stage().legality(),
        source.homes(),
        source.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        raw,
    )
}
pub(crate) fn named_units(staged: &StagedOptimizedLiveness, names: &[&str]) -> Vec<RegisterUnitId> {
    names
        .iter()
        .flat_map(|name| {
            staged
                .selected_stage()
                .register_environment()
                .physical()
                .model()
                .view_named(name)
                .unwrap()
                .units
                .iter()
                .copied()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
