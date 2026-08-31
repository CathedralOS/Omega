use crate::{
    PressureRematerializationPlan, PressureRematerializationValidationReceipt,
    pressure_rematerialization_identity,
};

pub(super) fn bind(
    plan: &PressureRematerializationPlan,
    transformed_selected: omega_selected_instructions::SelectedInstructionPlanIdentity,
    function_count: usize,
    applied_count: usize,
    rewritten_use_count: usize,
) -> PressureRematerializationValidationReceipt {
    PressureRematerializationValidationReceipt {
        identity: pressure_rematerialization_identity(plan),
        source_selected: plan.source_selected,
        spill_choices: plan.spill_choices,
        recovery_classifications: plan.recovery_classifications,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        transformed_selected,
        policy: plan.policy,
        usage: plan.usage,
        function_count,
        applied_count,
        rewritten_use_count,
    }
}
