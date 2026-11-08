use selected_instructions::{SelectedProgram, selected_instruction_plan_identity};

use crate::{
    FixedViewCopyPlan, FixedViewCopyValidationReceipt, ValidatedFixedViewCopies,
    fixed_view_copy_identity,
};

pub(crate) fn seal_validation(
    selected: &SelectedProgram,
    plan: FixedViewCopyPlan,
) -> ValidatedFixedViewCopies {
    let receipt = FixedViewCopyValidationReceipt {
        identity: fixed_view_copy_identity(&plan),
        source_selected: plan.source_selected,
        source_ranges: plan.source_ranges,
        source_legality: plan.source_legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        source_evidence: plan.source_evidence,
        transformed_selected: selected_instruction_plan_identity(&plan.transformed),
        optimization_unit: selected.optimization_unit(),
        fuel_schedule: selected.fuel_schedule(),
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.transformed.functions.len(),
        copy_count: plan.copies.len(),
    };
    ValidatedFixedViewCopies { plan, receipt }
}
