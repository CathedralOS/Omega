//! Ordinary Unit arguments retain their authored local and exact borrow event.

use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
};

use crate::{LoweringError, unsupported};

pub(in crate::attached_unit) fn validate(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    let (CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        structural_arguments,
        ..
    }
    | CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        target_machine,
        target_state,
        structural_arguments,
        ..
    }) = operation
    else {
        return unsupported("primitive local Unit custody requires an ordinary Unit call");
    };
    let authored =
        crate::call_source_custody::authored::locate_source(checked, caller.state, *coordinate)?;
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
    if machine.symbol != caller.machine
        || authored.target_machine != *target_machine
        || authored.target_state != *target_state
        || authored.boundary
        || structural_arguments.len() != target_parameters.len()
    {
        return unsupported("primitive local Unit call disagrees with its authored signature");
    }
    for (argument, parameter) in structural_arguments.iter().zip(target_parameters) {
        let retained_local = matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        );
        let Some(expression) =
            authored
                .structural_arguments
                .iter()
                .find_map(|(position, expression)| {
                    (*position == parameter.position).then_some(*expression)
                })
        else {
            if retained_local {
                return unsupported("primitive local Unit call lost its explicit authored actual");
            }
            continue;
        };
        let source = match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => expression,
        };
        let authored_local = match checked.expression_table.expression(source) {
            ExpressionNode::Name(name) => checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(coordinate.statement_index as usize)
                .any(|statement| {
                    matches!(statement, StatementNode::LocalData(local)
                        if local.symbol == name.symbol && local.is_mutable
                            && checked.primitive_type_reference(local.type_reference).is_some())
                }),
            _ => false,
        };
        // Inspect the authored operand as well as the retained source tag: changing
        // PrimitiveLocal to Parameter must not avoid the local's custody check.
        if !retained_local && !authored_local {
            continue;
        }
        super::super::retain_exact_checked_flow_call(checked, caller, *coordinate, *target_state)?;
        if !super::validate_argument_source(
            checked,
            caller,
            *coordinate,
            authored.source_target,
            argument,
            expression,
        )? {
            return unsupported("primitive local Unit actual changed its source owner");
        }
    }
    Ok(())
}
