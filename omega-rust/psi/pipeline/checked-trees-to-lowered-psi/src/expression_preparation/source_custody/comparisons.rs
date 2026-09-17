//! Exact selected comparison custody: the checked use's selected Terminal
//! meaning (IEEE float or authored-order integer), its statement owner and
//! its one boundary application, replayed from the checked facts rather than
//! trusted from the computation node.

use crate::emission::scalar_types::terminal_scalar_type;
use crate::emission::selected_comparison::{SelectedComparison, SelectedComparisonMeaning};
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::expression::BinaryOperator;
use checked_trees::types::PrimitiveType;
use lowered_psi::LoweredSelectedIntegerComparisonOperation;
use semantic_vocabulary::ScalarType;

pub(crate) fn occurrence(
    checked: &CheckedTrees,
    handle: checked_trees::CheckedOperatorUseHandle,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
) -> Result<SelectedComparison, LoweringError> {
    let meaning = selected_meaning(checked, handle)?;
    let selected = checked.facts.operators.uses.get(handle);
    validate_origin(checked, selected)?;
    if !matches!(selected.origin, checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, state_symbol, statement_index, .. }
        if machine_symbol == machine && state_symbol == state && usize::try_from(statement).ok() == Some(statement_index))
    {
        return unsupported("selected comparison lost its exact source owner");
    }
    // Omega rejoins the opaque commitment to the actual selected ProviderPlan;
    // a use that reached lowering without one has no provider to rejoin.
    if selected.provider_plan_commitment.is_empty()
        || selected.provider_plan_report_fingerprint == 0
    {
        return unsupported("selected comparison has no complete provider plan evidence");
    }
    if checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| {
            application.site == selected.application_site()
                && application.requirement_symbol == selected.selected_operator_symbol
        })
        .count()
        != 1
    {
        return unsupported("selected comparison does not rejoin one exact boundary application");
    }
    Ok(SelectedComparison {
        operator_use: handle,
        application_site: selected.application_site(),
        requirement_operator: selected.selected_operator_symbol,
        provider_plan_report_fingerprint: selected.provider_plan_report_fingerprint,
        provider_plan_commitment: selected.provider_plan_commitment,
        meaning,
    })
}

/// The one Terminal operation a selected comparison use denotes. The checked
/// classifiers decide what was selected; this only names the operation that
/// keeps the authored operand order, because the operation's positional
/// operand roster is the formal telescope operation crash contracts and
/// provider rejoins read. A use with no such operation fails closed rather
/// than lowering through a swapped or composed emission.
fn selected_meaning(
    checked: &CheckedTrees,
    handle: checked_trees::CheckedOperatorUseHandle,
) -> Result<SelectedComparisonMeaning, LoweringError> {
    let operators = &checked.facts.operators;
    if let Some((comparison, primitive)) =
        operators.selected_float_comparison(&checked.typed, handle)
    {
        let format = match primitive {
            PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
            PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
            _ => return unsupported("selected comparison has a non-float operand"),
        };
        return Ok(SelectedComparisonMeaning::IeeeFloat { comparison, format });
    }
    let Some((operation, primitive)) =
        operators.selected_integer_comparison(&checked.typed, handle)
    else {
        return unsupported("comparison has no exact selected Terminal meaning");
    };
    let comparison = match operation {
        BinaryOperator::Equal => LoweredSelectedIntegerComparisonOperation::Equal,
        BinaryOperator::Less => LoweredSelectedIntegerComparisonOperation::LessThan,
        BinaryOperator::LessOrEqual => LoweredSelectedIntegerComparisonOperation::LessOrEqual,
        _ => {
            return unsupported(
                "selected integer comparison has no authored-order Terminal operation to join",
            );
        }
    };
    let ScalarType::Integer(integer_type) = terminal_scalar_type(primitive)? else {
        return unsupported("selected integer comparison has a non-integer operand");
    };
    Ok(SelectedComparisonMeaning::Integer {
        comparison,
        integer_type,
    })
}

fn validate_origin(
    checked: &CheckedTrees,
    selected: &checked_trees::CheckedOperatorUseFact,
) -> Result<(), LoweringError> {
    use checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use checked_trees::{CheckedValueOrigin, CheckedValueStatementRole as Role};
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        role,
    } = selected.origin
    else {
        return unsupported("comparison has no executable statement origin");
    };
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .ok_or(LoweringError::Unsupported(
            "comparison owner machine is absent",
        ))?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .ok_or(LoweringError::Unsupported(
            "comparison owner state is absent",
        ))?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
        .ok_or(LoweringError::Unsupported(
            "comparison owner statement is absent",
        ))?;
    let mut roots = Vec::new();
    match (statement, role) {
        (StatementNode::Expression(expression), Role::Expression) => roots.push(*expression),
        (StatementNode::LocalData(local), Role::LocalInitializer) => {
            roots.push(local.initial_value)
        }
        (StatementNode::Assignment(assignment), Role::AssignmentValue) => {
            roots.push(assignment.value)
        }
        (StatementNode::Assignment(assignment), Role::AssignmentTargetSubexpression) => {
            roots.push(assignment.target)
        }
        (StatementNode::Call(call), Role::CallArgument) => {
            roots.extend_from_slice(program.statement_table.expression_handles(call.arguments))
        }
        (StatementNode::Transition(transition), Role::TransitionGuard) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                roots.push(expression);
            }
        }
        (
            StatementNode::Transition(transition),
            Role::TransitionTargetArgument | Role::TransitionTargetValue,
        ) => {
            for target in [transition.target, transition.continuation] {
                match (program.statement_table.transition_target(target), role) {
                    (TransitionTargetNode::Value(expression), Role::TransitionTargetValue) => {
                        roots.push(*expression)
                    }
                    (
                        TransitionTargetNode::Named { arguments, .. },
                        Role::TransitionTargetArgument,
                    ) => roots
                        .extend_from_slice(program.statement_table.expression_handles(*arguments)),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    for root in roots {
        if crate::expression_preparation::source_custody::computation_calls::authored_expressions(
            checked, root,
        )?
        .contains(&selected.expression)
        {
            return Ok(());
        }
    }
    unsupported("comparison expression escaped its exact authored origin role")
}
