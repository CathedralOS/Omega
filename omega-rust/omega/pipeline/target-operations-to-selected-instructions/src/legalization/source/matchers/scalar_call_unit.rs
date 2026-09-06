use super::super::shared::*;
use crate::legalization::catalog::{
    LEGALIZATION_FORMS, LegalizationFormDescriptor, LegalizationProducerMatcherKind,
    ScalarCallUnitLegalizationMatcherKind,
};

pub(in crate::legalization::source) fn match_scalar_call_unit_form(
    target: &target_operations::TargetFunction,
) -> Option<&'static LegalizationFormDescriptor> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return None;
    };
    let Some((TargetUnitOperation::Return { .. }, operations)) = body.operations.split_last()
    else {
        return None;
    };
    if !operations
        .iter()
        .any(|operation| matches!(operation, TargetUnitOperation::ScalarCall { .. }))
        || operations.iter().any(|operation| {
            !matches!(
                operation,
                TargetUnitOperation::IntegerConstant { .. }
                    | TargetUnitOperation::ScalarCall { .. }
            )
        })
    {
        return None;
    }
    LEGALIZATION_FORMS.iter().find(|form| {
        form.producer_matcher
            == LegalizationProducerMatcherKind::ScalarCallUnit(
                ScalarCallUnitLegalizationMatcherKind::OrderedU64RegisterCalls,
            )
    })
}
