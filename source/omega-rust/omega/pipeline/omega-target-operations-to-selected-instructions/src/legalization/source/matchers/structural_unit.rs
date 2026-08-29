use crate::legalization::catalog::{
    LEGALIZATION_FORMS, LegalizationFormDescriptor, LegalizationProducerMatcherKind,
    LegalizationShapeConstraints, StructuralUnitLegalizationMatcherKind,
};

use super::super::shared::*;
use super::unit::is_plain_unit_contract;

pub(crate) struct MatchedStructuralUnitForm<'a> {
    pub descriptor: &'static LegalizationFormDescriptor,
    pub target_call: Option<&'a TargetUnitOperation>,
    pub target_return: &'a TargetUnitOperation,
    pub abstract_call: Option<&'a AbstractOperation>,
    pub abstract_return: &'a AbstractOperation,
    pub optimized_call: Option<&'a omega_optimization_unit::OptimizationNode>,
    pub optimized_return: &'a omega_optimization_unit::OptimizationNode,
    pub settlement_rows: Option<(
        &'a [TargetUnitOperation],
        &'a [AbstractOperation],
        &'a [omega_optimization_unit::OptimizationNode],
    )>,
}

pub(crate) fn match_structural_unit_form<'a>(
    target: &'a omega_target_operations::TargetFunction,
    abstracted: &'a omega_abstract_operations::AbstractFunction,
    optimized: &'a omega_optimization_unit::PsiOptimizationFunction,
) -> Option<MatchedStructuralUnitForm<'a>> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return None;
    };
    if is_plain_unit_contract(target, abstracted, optimized) {
        return None;
    }
    let [optimized_block] = optimized.blocks.as_slice() else {
        return None;
    };
    let mut matches = LEGALIZATION_FORMS.iter().filter_map(|descriptor| {
        let (
            LegalizationProducerMatcherKind::StructuralUnit(matcher),
            LegalizationShapeConstraints::StructuralUnit(constraints),
        ) = (descriptor.producer_matcher, descriptor.constraints)
        else {
            return None;
        };
        (abstracted.block_entries.len() == constraints.block_count
            && optimized.blocks.len() == constraints.block_count
            && abstracted.parameters.len() == constraints.scalar_parameter_count
            && optimized.parameters.len() == constraints.scalar_parameter_count)
            .then_some(())
            .and_then(|()| {
                match_form(
                    descriptor,
                    matcher,
                    &body.operations,
                    &abstracted.operations,
                    &optimized_block.nodes,
                )
            })
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

fn match_form<'a>(
    descriptor: &'static LegalizationFormDescriptor,
    matcher: StructuralUnitLegalizationMatcherKind,
    target: &'a [TargetUnitOperation],
    abstracted: &'a [AbstractOperation],
    optimized: &'a [omega_optimization_unit::OptimizationNode],
) -> Option<MatchedStructuralUnitForm<'a>> {
    let (
        target_call,
        target_return,
        abstract_call,
        abstract_return,
        optimized_call,
        optimized_return,
        settlement_rows,
    ) = match matcher {
        StructuralUnitLegalizationMatcherKind::ReturnUnit => {
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
        StructuralUnitLegalizationMatcherKind::AuthoredCallThenReturnUnit => {
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
        StructuralUnitLegalizationMatcherKind::InstalledProviderCallThenReturnUnit => {
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
        StructuralUnitLegalizationMatcherKind::ClaimCompletionSettlementsThenReturnUnit => {
            match (target, abstracted, optimized) {
                (
                    [target_settlements @ .., target_return @ TargetUnitOperation::Return { .. }],
                    [abstract_settlements @ .., abstract_return @ AbstractOperation::ReturnUnit { .. }],
                    [optimized_settlements @ .., optimized_return],
                ) if !target_settlements.is_empty()
                    && target_settlements.len() == abstract_settlements.len()
                    && target_settlements.len() == optimized_settlements.len()
                    && target_settlements.iter().all(|operation| matches!(
                        operation,
                        TargetUnitOperation::BoundarySettlement {
                            realization: omega_target_operations::BoundaryRealization::ClaimCompletionOnly(_),
                            ..
                        }
                    ))
                    && abstract_settlements.iter().all(|operation| {
                        matches!(operation, AbstractOperation::BoundaryCall { .. })
                    }) => (
                        None,
                        target_return,
                        None,
                        abstract_return,
                        None,
                        optimized_return,
                        Some((target_settlements, abstract_settlements, optimized_settlements)),
                    ),
                _ => return None,
            }
        }
    };
    Some(MatchedStructuralUnitForm {
        descriptor,
        target_call,
        target_return,
        abstract_call,
        abstract_return,
        optimized_call,
        optimized_return,
        settlement_rows,
    })
}
