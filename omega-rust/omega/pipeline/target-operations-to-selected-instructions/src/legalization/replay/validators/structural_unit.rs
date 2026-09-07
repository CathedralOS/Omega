use crate::legalization::catalog::{
    LegalizationFormRecipe, LegalizationShapeConstraints, LegalizationValidatorKind,
    StructuralUnitLegalizationValidatorKind, legalization_form_for_recipe,
};

use super::super::shared::*;

pub(crate) struct ValidatedStructuralUnitForm<'a> {
    pub target_call: Option<&'a TargetUnitOperation>,
    pub target_return: &'a TargetUnitOperation,
    pub abstract_call: Option<&'a AbstractOperation>,
    pub abstract_return: &'a AbstractOperation,
    pub optimized_call: Option<&'a optimization_unit::OptimizationNode>,
    pub optimized_return: &'a optimization_unit::OptimizationNode,
    pub settlement_rows: Option<(
        &'a [TargetUnitOperation],
        &'a [AbstractOperation],
        &'a [optimization_unit::OptimizationNode],
    )>,
}

pub(crate) fn validate_structural_unit_form<'a>(
    target: &'a target_operations::TargetFunction,
    abstracted: &'a abstract_operations::AbstractFunction,
    optimized: &'a optimization_unit::PsiOptimizationFunction,
    recipe: legalized_operations::StructuralUnitLegalizationRecipe,
) -> Option<ValidatedStructuralUnitForm<'a>> {
    if independently_plain_unit_contract(target, abstracted, optimized) {
        return None;
    }
    let descriptor = legalization_form_for_recipe(LegalizationFormRecipe::StructuralUnit(recipe))?;
    let (
        LegalizationValidatorKind::StructuralUnit(validator),
        LegalizationShapeConstraints::StructuralUnit(constraints),
    ) = (descriptor.validator, descriptor.constraints)
    else {
        return None;
    };
    let TargetOperation::UnitBody(body) = &target.operation else {
        return None;
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        return None;
    };
    if abstracted.block_entries.len() != constraints.block_count
        || optimized.blocks.len() != constraints.block_count
        || abstracted.parameters.len() != constraints.scalar_parameter_count
        || optimized.parameters.len() != constraints.scalar_parameter_count
    {
        return None;
    }
    validate_form(
        validator,
        &body.operations,
        &abstracted.operations,
        &optimized_block.nodes,
    )
}

fn validate_form<'a>(
    validator: StructuralUnitLegalizationValidatorKind,
    target: &'a [TargetUnitOperation],
    abstracted: &'a [AbstractOperation],
    optimized: &'a [optimization_unit::OptimizationNode],
) -> Option<ValidatedStructuralUnitForm<'a>> {
    let (
        target_call,
        target_return,
        abstract_call,
        abstract_return,
        optimized_call,
        optimized_return,
        settlement_rows,
    ) = match validator {
        StructuralUnitLegalizationValidatorKind::ReturnUnit => {
            match (target, abstracted, optimized) {
                (
                    [target_return @ TargetUnitOperation::Return { .. }],
                    [abstract_return @ AbstractOperation::ReturnUnit { .. }],
                    [optimized_return],
                ) => (
                    None,
                    target_return,
                    None,
                    abstract_return,
                    None,
                    optimized_return,
                    None,
                ),
                _ => return None,
            }
        }
        StructuralUnitLegalizationValidatorKind::AuthoredCallThenReturnUnit => {
            match (target, abstracted, optimized) {
                (
                    [
                        target_call @ TargetUnitOperation::Call { .. },
                        target_return @ TargetUnitOperation::Return { .. },
                    ],
                    [
                        abstract_call @ AbstractOperation::CallUnit { .. },
                        abstract_return @ AbstractOperation::ReturnUnit { .. },
                    ],
                    [optimized_call, optimized_return],
                ) => (
                    Some(target_call),
                    target_return,
                    Some(abstract_call),
                    abstract_return,
                    Some(optimized_call),
                    optimized_return,
                    None,
                ),
                _ => return None,
            }
        }
        StructuralUnitLegalizationValidatorKind::InstalledProviderCallThenReturnUnit => {
            match (target, abstracted, optimized) {
                (
                    [
                        target_call @ TargetUnitOperation::InstalledProviderCall { .. },
                        target_return @ TargetUnitOperation::Return { .. },
                    ],
                    [
                        abstract_call @ AbstractOperation::BoundaryCall { .. },
                        abstract_return @ AbstractOperation::ReturnUnit { .. },
                    ],
                    [optimized_call, optimized_return],
                ) => (
                    Some(target_call),
                    target_return,
                    Some(abstract_call),
                    abstract_return,
                    Some(optimized_call),
                    optimized_return,
                    None,
                ),
                _ => return None,
            }
        }
        StructuralUnitLegalizationValidatorKind::ClaimCompletionSettlementsThenReturnUnit => {
            match (target, abstracted, optimized) {
                (
                    [
                        target_settlements @ ..,
                        target_return @ TargetUnitOperation::Return { .. },
                    ],
                    [
                        abstract_settlements @ ..,
                        abstract_return @ AbstractOperation::ReturnUnit { .. },
                    ],
                    [optimized_settlements @ .., optimized_return],
                ) if !target_settlements.is_empty()
                    && target_settlements.len() == abstract_settlements.len()
                    && target_settlements.len() == optimized_settlements.len()
                    && target_settlements.iter().all(|operation| {
                        matches!(
                            operation,
                            TargetUnitOperation::BoundarySettlement {
                                realization:
                                    target_operations::BoundaryRealization::ClaimCompletionOnly(_),
                                ..
                            }
                        )
                    })
                    && abstract_settlements.iter().all(|operation| {
                        matches!(operation, AbstractOperation::BoundaryCall { .. })
                    }) =>
                {
                    (
                        None,
                        target_return,
                        None,
                        abstract_return,
                        None,
                        optimized_return,
                        Some((
                            target_settlements,
                            abstract_settlements,
                            optimized_settlements,
                        )),
                    )
                }
                _ => return None,
            }
        }
    };
    Some(ValidatedStructuralUnitForm {
        target_call,
        target_return,
        abstract_call,
        abstract_return,
        optimized_call,
        optimized_return,
        settlement_rows,
    })
}

fn independently_plain_unit_contract(
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
