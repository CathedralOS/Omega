use super::super::shared::*;
use crate::legalization::catalog::{
    LEGALIZATION_FORMS, LegalizationFormDescriptor, LegalizationProducerMatcherKind,
    ScalarCallUnitLegalizationMatcherKind,
};

pub(in crate::legalization::source) fn match_scalar_call_unit_form(
    target: &omega_target_operations::TargetFunction,
) -> Option<&'static LegalizationFormDescriptor> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return None;
    };
    let [
        TargetUnitOperation::IntegerConstant { .. },
        TargetUnitOperation::IntegerConstant { .. },
        TargetUnitOperation::ScalarCall { .. },
        TargetUnitOperation::ScalarCall { .. },
        TargetUnitOperation::ScalarCall { .. },
        TargetUnitOperation::Return { .. },
    ] = body.operations.as_slice()
    else {
        return None;
    };
    LEGALIZATION_FORMS.iter().find(|form| {
        form.producer_matcher
            == LegalizationProducerMatcherKind::ScalarCallUnit(
                ScalarCallUnitLegalizationMatcherKind::U64EqualityConditionalThreeCallChain,
            )
    })
}
