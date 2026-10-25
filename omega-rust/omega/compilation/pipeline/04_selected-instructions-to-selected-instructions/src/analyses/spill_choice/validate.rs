use crate::{SpillChoiceError, SpillChoiceValidationReceipt, ValidatedSpillChoices};
use register_homes::{SpillChoicePlan, spill_choice_identity};

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_spill_choices(
    plan: SpillChoicePlan,
) -> Result<ValidatedSpillChoices, SpillChoiceError> {
    let receipt = SpillChoiceValidationReceipt {
        identity: spill_choice_identity(&plan),
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        policy: plan.policy,
        usage: plan.usage,
    };
    Ok(ValidatedSpillChoices { plan, receipt })
}
