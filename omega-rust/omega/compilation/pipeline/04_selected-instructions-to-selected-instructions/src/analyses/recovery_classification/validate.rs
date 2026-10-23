use crate::{
    RecoveryClassificationError, RecoveryClassificationValidationReceipt,
    ValidatedRecoveryClassifications,
};
use register_homes::{
    RecoveryClassification, RecoveryClassificationPlan, recovery_classification_identity,
};

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output, and the work accounting this plan was
/// produced under already admitted its exact total, so this seal does not
/// repeat that admission.
pub(crate) fn seal_recovery_classifications(
    plan: RecoveryClassificationPlan,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    let classification_count = plan
        .functions
        .iter()
        .filter(|function| function.classification.is_some())
        .count();
    let immediate_candidate_count = plan
        .functions
        .iter()
        .filter(|function| {
            function.classification.as_ref().is_some_and(|row| {
                matches!(
                    &row.classification,
                    RecoveryClassification::ImmediateU64RematerializationCandidate { .. }
                )
            })
        })
        .count();
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
        function_count: plan.functions.len(),
        classification_count,
        immediate_candidate_count,
    };
    Ok(ValidatedRecoveryClassifications { plan, receipt })
}
