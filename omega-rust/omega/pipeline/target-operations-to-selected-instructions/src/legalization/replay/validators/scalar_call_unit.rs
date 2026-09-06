use super::super::shared::*;
use crate::legalization::catalog::{
    LegalizationFormRecipe, LegalizationValidatorKind, ScalarCallUnitLegalizationValidatorKind,
    legalization_form_for_recipe,
};
use legalized_operations::ScalarCallUnitLegalizationRecipe;

pub(in crate::legalization::replay) fn validate_scalar_call_unit_form(
    target: &target_operations::TargetFunction,
    recipe: ScalarCallUnitLegalizationRecipe,
) -> bool {
    let Some(form) = legalization_form_for_recipe(LegalizationFormRecipe::ScalarCallUnit(recipe))
    else {
        return false;
    };
    if form.validator
        != LegalizationValidatorKind::ScalarCallUnit(
            ScalarCallUnitLegalizationValidatorKind::OrderedU64PairCalls,
        )
    {
        return false;
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return false;
    };
    let Some((TargetUnitOperation::Return { .. }, operations)) = body.operations.split_last()
    else {
        return false;
    };
    operations
        .iter()
        .any(|operation| matches!(operation, TargetUnitOperation::ScalarCall { .. }))
        && operations.iter().all(|operation| {
            matches!(
                operation,
                TargetUnitOperation::IntegerConstant { .. }
                    | TargetUnitOperation::ScalarCall { .. }
            )
        })
}
