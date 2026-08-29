use omega_selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};

use crate::{
    LiteralFoldPlan, LiteralFoldValidationReceipt, ValidatedLiteralFold, literal_fold_identity,
};

pub(super) fn admit_literal_fold(
    plan: LiteralFoldPlan,
    transformed: SelectedInstructionPlan,
    transformed_selected: SelectedInstructionPlanIdentity,
    applied_count: usize,
) -> ValidatedLiteralFold {
    let receipt = LiteralFoldValidationReceipt {
        identity: literal_fold_identity(&plan),
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
        function_count: transformed.functions.len(),
        applied_count,
    };
    ValidatedLiteralFold {
        plan,
        transformed,
        receipt,
    }
}
