

use crate::{
    SpillChoiceError, SpillChoiceValidationReceipt, ValidatedSpillChoices,
};
use register_homes::{
    SpillChoicePlan, spill_choice_identity,
};



/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_spill_choices(
    plan: SpillChoicePlan,
) -> Result<ValidatedSpillChoices, SpillChoiceError> {
    let choice_count = plan
        .functions
        .iter()
        .filter(|function| function.choice.is_some())
        .count();
    let contender_count = plan
        .functions
        .iter()
        .filter_map(|function| function.choice.as_ref())
        .map(|choice| choice.contenders.len())
        .sum();
    let receipt = SpillChoiceValidationReceipt {
        identity: spill_choice_identity(&plan),
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        choice_count,
        contender_count,
    };
    Ok(ValidatedSpillChoices { plan, receipt })
}

