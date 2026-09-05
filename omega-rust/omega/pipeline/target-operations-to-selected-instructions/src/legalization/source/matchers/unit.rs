use crate::legalization::catalog::{
    LEGALIZATION_FORMS, LegalizationFormDescriptor, LegalizationProducerMatcherKind,
    LegalizationShapeConstraints, UnitLegalizationMatcherKind,
};

use super::super::shared::*;

pub(crate) fn match_unit_form(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> Option<&'static LegalizationFormDescriptor> {
    let mut matches = LEGALIZATION_FORMS.iter().filter(|descriptor| {
        let (
            LegalizationProducerMatcherKind::Unit(UnitLegalizationMatcherKind::ReturnUnit),
            LegalizationShapeConstraints::Unit(constraints),
        ) = (descriptor.producer_matcher, descriptor.constraints)
        else {
            return false;
        };
        is_plain_unit_contract(target, abstracted, optimized)
            && abstracted.block_entries.len() == constraints.block_count
            && optimized.blocks.len() == constraints.block_count
            && abstracted.operations.len() == constraints.operation_count
            && optimized
                .blocks
                .first()
                .is_some_and(|block| block.nodes.len() == constraints.node_count)
            && abstracted.parameters.len() == constraints.scalar_parameter_count
            && optimized.parameters.len() == constraints.scalar_parameter_count
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

pub(super) fn is_plain_unit_contract(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
) -> bool {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return false;
    };
    body.parameters.is_empty()
        && abstracted.structural_parameters.is_empty()
        && optimized.structural_parameters.is_empty()
        && abstracted.entry_claims.is_empty()
        && optimized.entry_claim_declarations.is_empty()
        && optimized.entry_claims.is_empty()
        && optimized.declared_places.is_empty()
        && abstracted.published_service_ceiling.is_empty()
        && optimized.published_service_ceiling.is_empty()
        && matches!(
            body.operations.as_slice(),
            [TargetUnitOperation::Return { .. }]
        )
}
