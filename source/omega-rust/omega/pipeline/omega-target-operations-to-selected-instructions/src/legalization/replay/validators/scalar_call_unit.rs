use super::super::shared::*;
use crate::legalization::catalog::{
    LegalizationFormRecipe, LegalizationValidatorKind, ScalarCallUnitLegalizationValidatorKind,
    legalization_form_for_recipe,
};
use omega_legalized_operations::ScalarCallUnitLegalizationRecipe;

pub(in crate::legalization::replay) fn validate_scalar_call_unit_form(
    target: &omega_target_operations::TargetFunction,
    recipe: ScalarCallUnitLegalizationRecipe,
) -> bool {
    let Some(form) = legalization_form_for_recipe(LegalizationFormRecipe::ScalarCallUnit(recipe))
    else {
        return false;
    };
    if form.validator
        != LegalizationValidatorKind::ScalarCallUnit(
            ScalarCallUnitLegalizationValidatorKind::U64EqualityConditionalThreeCallChain,
        )
    {
        return false;
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return false;
    };
    matches!(
        body.operations.as_slice(),
        [
            TargetUnitOperation::IntegerConstant { .. },
            TargetUnitOperation::IntegerConstant { .. },
            TargetUnitOperation::ScalarCall { .. },
            TargetUnitOperation::ScalarCall { .. },
            TargetUnitOperation::ScalarCall { .. },
            TargetUnitOperation::Return { .. },
        ]
    )
}
