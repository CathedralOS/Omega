//! Contract call and exit obligations under the shared current-value transfer.
//! The flow owner selects the exact operation and invocation origins; the
//! arithmetic roster supplies only the values observed at that program point.

use crate::checks::current_scalar_values::CurrentScalarValues;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{StatementNode, TransitionTargetHandle};

/// Current-state proof only. The exit owner separately requires exact incoming
/// origins for every immutable invocation formal named by the guarantee.
pub(crate) fn proves_exit(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    exit: &checked_trees::FlowExitFact,
    fact: &facts::Fact,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let facts::FactPayload::ContractBooleanExpression {
        fact: contract,
        expression,
        ..
    } = fact.payload
    else {
        return false;
    };
    let Some((machine, state)) = program.state_flow_site(exit.machine_symbol, exit.state_symbol)
    else {
        return false;
    };
    let Some(values) = CurrentScalarValues::at(
        program,
        machine,
        state,
        exit.statement_index,
        exit.transition_target,
        frames,
    ) else {
        return false;
    };
    let Some(correspondence) = crate::semantic::facts::selected_contract_parameter_correspondence(
        program,
        &facts.proof,
        exit.ensures,
        contract,
    ) else {
        return false;
    };
    let returned = match program
        .statement_table
        .statement_at(state, exit.statement_index)
    {
        Some(StatementNode::Expression(expression)) if !exit.transition_target.is_valid() => {
            *expression
        }
        Some(StatementNode::Transition(_)) if exit.transition_target.is_valid() => {
            let typed_trees::statement::TransitionTargetNode::Value(expression) = program
                .statement_table
                .transition_target(exit.transition_target)
            else {
                return false;
            };
            *expression
        }
        _ => return false,
    };
    let Some(goal) = values.bind_exit_contract(
        &correspondence,
        facts
            .flow
            .control
            .exit_parameter_origins
            .span_or_empty(exit.parameter_origins),
        contract,
        expression,
        returned,
    ) else {
        return false;
    };
    values.proves(program, machine, &goal)
}

/// Bind the exact call's formal roster simultaneously to current actuals.
/// The same prefix transfer handles an ordinary call and a named state edge.
pub(crate) fn proves_call(
    program: &dyn ::validation::CheckingInput,
    state_flow: &checked_trees::FlowStateFact,
    call: &checked_trees::FlowCallFact,
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((machine, state)) =
        program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return false;
    };
    let Some(site) = validation::find_source_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return false;
    };
    let Some(source_target) = site.source_target_symbol(program, call.target_symbol) else {
        return false;
    };
    let body = validation::find_state(program, source_target)
        .or_else(|| validation::find_machine_head(program, source_target));
    let (callee_owner, parameters) = if let Some((_, callee_state)) = body {
        (callee_state.symbol, program.state_parameters(callee_state))
    } else {
        let application = match &site {
            validation::CallSite::Expression { call: selected, .. }
                if selected.target_symbol == source_target =>
            {
                selected.authored_machine_application
            }
            validation::CallSite::Statement(selected)
                if selected.target_symbol == source_target =>
            {
                selected.authored_machine_application
            }
            _ => return false,
        };
        let Some((definition, signature)) =
            program.selected_trait_requirement_signature(source_target, application)
        else {
            return false;
        };
        if !definition.is_boundary {
            return false;
        }
        (
            signature.symbol,
            program.state_signature_parameters(signature),
        )
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    if arguments.len() != parameters.len() {
        return false;
    }
    let target = if let validation::CallSite::TransitionNamed {
        path, arguments, ..
    } = &site
    {
        let Some(StatementNode::Transition(transition)) = program
            .statement_table
            .statement_at(state, call.statement_index)
        else {
            return false;
        };
        let Some(target) = [transition.target, transition.continuation].into_iter().find(|target| target.is_valid() && matches!(program.statement_table.transition_target(*target),
            typed_trees::statement::TransitionTargetNode::Named { path: candidate, arguments: actuals, .. }
                if candidate.symbol == path.symbol && actuals == arguments)) else { return false; };
        target
    } else {
        TransitionTargetHandle::invalid()
    };
    let Some(mut values) = CurrentScalarValues::at(
        program,
        machine,
        state,
        call.statement_index,
        target,
        frames,
    ) else {
        return false;
    };
    if !target.is_valid() {
        let Some(statement) = program
            .statement_table
            .statement_at(state, call.statement_index)
        else {
            return false;
        };
        // The ordinal identifies a call inside this statement, but its earlier
        // sibling operands may already have changed storage. Reuse the full
        // operand frame conservatively until an ordered capture is available.
        values.invalidate_operand_writes(
            frames.map(|frames| frames.statement_value_write_frame(machine, statement)),
        );
    }
    let Some(goal) = values.bind_parameters(callee_owner, body, expression, parameters, arguments)
    else {
        return false;
    };
    values.proves(program, machine, &goal)
}
