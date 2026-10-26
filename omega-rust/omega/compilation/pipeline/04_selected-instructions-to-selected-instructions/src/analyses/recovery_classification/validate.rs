use crate::{
    RecoveryClassificationError, RecoveryClassificationValidationReceipt,
    ValidatedRecoveryClassifications,
};
use register_homes::{RecoveryClassificationPlan, recovery_classification_identity};

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output, and the work accounting this plan was
/// produced under already admitted its exact total, so this seal does not
/// repeat that admission.
pub(crate) fn seal_recovery_classifications(
    plan: RecoveryClassificationPlan,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    let receipt = RecoveryClassificationValidationReceipt {
        identity: recovery_classification_identity(&plan),
        selected: plan.selected,
        spill_choices: plan.spill_choices,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        policy: plan.policy,
        usage: plan.usage,
    };
    Ok(ValidatedRecoveryClassifications { plan, receipt })
}
