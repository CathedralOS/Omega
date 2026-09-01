//! Ordered whole-plan family inventory and exact-zero-or-one replay dispatch.

use omega_abstract_operations::AbstractOperationPlan;
use omega_target_operations::TargetOperationPlan;

use crate::validation::{
    AbstractToTargetPlanTranslationFamily, AbstractToTargetTranslationValidationError,
    StructuralCallReturnProjectedQualificationReceipt, structural_call_return,
};

#[derive(Clone, Copy)]
struct PlanTranslationFamilyDescriptor {
    family: AbstractToTargetPlanTranslationFamily,
    is_candidate: fn(&AbstractOperationPlan) -> bool,
    validate: fn(
        &AbstractOperationPlan,
        &TargetOperationPlan,
    ) -> Result<
        StructuralCallReturnProjectedQualificationReceipt,
        crate::validation::StructuralCallReturnProjectedQualificationValidationError,
    >,
}

const ENABLED_PLAN_TRANSLATION_FAMILIES: &[PlanTranslationFamilyDescriptor] =
    &[PlanTranslationFamilyDescriptor {
        family: AbstractToTargetPlanTranslationFamily::StructuralCallReturnProjectedQualifications,
        is_candidate: structural_call_return::is_candidate,
        validate: structural_call_return::validate,
    }];

pub(in crate::validation) fn validate(
    source: &AbstractOperationPlan,
    target: &TargetOperationPlan,
) -> Result<
    Option<StructuralCallReturnProjectedQualificationReceipt>,
    AbstractToTargetTranslationValidationError,
> {
    let mut selected = None;
    for descriptor in ENABLED_PLAN_TRANSLATION_FAMILIES {
        if !(descriptor.is_candidate)(source) {
            continue;
        }
        if selected.is_some() {
            return Err(AbstractToTargetTranslationValidationError::AmbiguousPlanFamily);
        }
        selected = Some(descriptor);
    }
    let Some(descriptor) = selected else {
        return Ok(None);
    };
    (descriptor.validate)(source, target)
        .map(Some)
        .map_err(
            |error| AbstractToTargetTranslationValidationError::PlanFamily {
                family: descriptor.family,
                error,
            },
        )
}
