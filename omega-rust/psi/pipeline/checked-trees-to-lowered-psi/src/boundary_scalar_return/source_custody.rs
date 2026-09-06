//! Authored result binding and return custody for the boundary-return route.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let [
        StatementNode::LocalData(local),
        StatementNode::Expression(returned),
    ] = program.statement_table.statements(state.statement_nodes)
    else {
        return unsupported("boundary scalar return lost its authored initializer and return");
    };
    if machine.symbol != plan.machine
        || program.machine_states(machine).len() != 1
        || local.is_mutable
        || !local.symbol.is_valid()
        || program.primitive_type_reference(local.type_reference) != Some(plan.result_type)
        || program.primitive_type_reference(state.return_type) != Some(plan.result_type)
        || plan.return_statement_ordinal != 1
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !program.expression_table.expression_is_valid(*returned)
    {
        return unsupported(
            "boundary scalar result disagrees with its authored binding or carrier",
        );
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(*returned) else {
        return unsupported("boundary scalar return is not its established result local");
    };
    if name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("boundary scalar return names another source value");
    }
    let (binding, expression) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            plan.state,
            plan.return_statement_ordinal,
            CheckedScalarExpressionRole::Return,
        )
        .ok_or(LoweringError::Unsupported(
            "boundary scalar return has no unique source-bound value",
        ))?;
    crate::scalar_source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(plan.result_type)?,
    )?;
    let returns_local = match expression {
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type,
        } => *primitive_type == plan.result_type,
        CheckedScalarExpression::Boolean(expression) => {
            plan.result_type == PrimitiveType::Bool
                && matches!(
                    expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Local { position: 0 }
                )
        }
        _ => false,
    };
    if !returns_local {
        return unsupported("boundary scalar return plan does not read its established result");
    }
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        scalar_arguments,
        ..
    } = &plan.boundary_call
    else {
        return unsupported("boundary scalar return has no boundary operation");
    };
    if scalar_arguments.iter().any(|argument| {
        matches!(
            argument,
            checked_trees::CheckedCallScalarArgument::Computation(_)
        )
    }) {
        crate::call_source_custody::initializers::validate(
            checked,
            plan.machine,
            plan.state,
            *coordinate,
        )?;
    }
    Ok(())
}
