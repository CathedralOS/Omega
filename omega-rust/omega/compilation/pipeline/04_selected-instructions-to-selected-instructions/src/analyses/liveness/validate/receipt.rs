//! Projects the validated liveness receipt and admits the exact validated liveness plan.

use crate::analyses::liveness::{LivenessValidationReceipt, ValidatedLiveness};
use selected_instructions::{
    LivenessPlan, liveness_function_digest, liveness_identity_from_digests,
};

pub(crate) fn admit_validated_liveness(
    plan: LivenessPlan,
    carried: &[bool],
    previous_digests: &[[u8; 32]],
) -> ValidatedLiveness {
    let function_digests =
        crate::analyses::carried_digests(&plan.functions, carried, previous_digests, |function| {
            liveness_function_digest(function)
        });
    let receipt = LivenessValidationReceipt {
        identity: liveness_identity_from_digests(&plan, &function_digests),
        selected: plan.selected,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
    };
    ValidatedLiveness {
        plan: plan.into(),
        receipt,
        function_digests,
    }
}
