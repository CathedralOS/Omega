//! Independent comparison of graph rows against source nodes and canonical ABI inputs.
use super::shared::*;
use crate::legalization::scalar_graph_input;
use ::legalized_operations::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    proposed: &LegalizedScalarFunction,
) -> Result<(), LegalizationError> {
    let input = scalar_graph_input::match_input(target, abstracted, optimized, native, plan, unit)?;
    let invalid = Error::NonCanonicalLegalizedPlan;
    let [block] = proposed.blocks.as_slice() else {
        return Err(invalid);
    };
    if proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.call_plan != input.call_plan
        || proposed.entry_block != input.block.id
        || block.id != input.block.id
        || proposed.parameters.len() != optimized.parameters.len()
        || block.instructions.len() != input.body.len()
        || proposed
            .parameters
            .iter()
            .zip(&optimized.parameters)
            .zip(&input.call_plan.parameters)
            .any(|((actual, source), placement)| {
                actual.value != source.value
                    || actual.scalar_type != scalar_graph_input::u64_type()
                    || actual.definition_site != source.site
                    || actual.placement != *placement
            })
    {
        return Err(invalid);
    }
    for (actual, node) in block.instructions.iter().zip(input.body) {
        let (operation, result) = scalar_graph_input::instruction(node).ok_or(invalid.clone())?;
        if actual.operation != operation
            || actual.result != result
            || actual.scalar_type != scalar_graph_input::u64_type()
            || actual.definition_site != node.definitions[0].site
            || actual.fuel != node.fuel
            || actual.effect != node.effect
            || actual.ownership != node.ownership
        {
            return Err(invalid);
        }
        match (&actual.kind, &node.operation) {
            (
                LegalizedScalarInstructionKind::Constant(actual),
                AbstractOperation::IntegerConstant { value, .. },
            ) if actual == value => {}
            (
                LegalizedScalarInstructionKind::Call(call),
                AbstractOperation::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                    ..
                },
            ) => {
                let expected = scalar_graph_input::callee_plan(*callee, native, plan, unit)?;
                if call.callee != *callee
                    || call.call_plan != expected
                    || Some(&call.result_placement) != expected.result.as_ref()
                    || call.requirement_obligations != *requirement_obligations
                    || call.crash_continuations != *crash_continuations
                    || call.arguments.len() != arguments.len()
                    || call
                        .arguments
                        .iter()
                        .zip(arguments)
                        .zip(&expected.parameters)
                        .any(|((actual, source), placement)| {
                            actual.source != *source || actual.placement != *placement
                        })
                    || proposed_plan
                        .scalar_functions
                        .iter()
                        .filter(|function| function.machine == *callee)
                        .count()
                        + proposed_plan
                            .functions
                            .iter()
                            .filter(|function| function.machine() == *callee)
                            .count()
                        != 1
                {
                    return Err(invalid);
                }
            }
            _ => return Err(invalid),
        }
    }
    let returned = &block.terminator;
    if returned.fuel != input.returned.fuel
        || returned.effect != input.returned.effect
        || returned.ownership != input.returned.ownership
    {
        return Err(invalid);
    }
    match (&returned.value, &input.returned.operation) {
        (LegalizedScalarReturnValue::Unit, AbstractOperation::ReturnUnit { psi_edge, .. })
            if returned.edge == *psi_edge => {}
        (
            LegalizedScalarReturnValue::Value { value, scalar_type },
            AbstractOperation::Return {
                psi_edge,
                value: source,
                ..
            },
        ) if returned.edge == *psi_edge
            && value == source
            && *scalar_type == scalar_graph_input::u64_type() => {}
        _ => return Err(invalid),
    }
    Ok(())
}
