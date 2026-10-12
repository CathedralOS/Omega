//! Checked whole-call transfers share the ordinary state jump vocabulary.
//! Source calls remain intact. Flow publishes one call row and saves its
//! actuals before recording this physical relation; this reader consumes that
//! same cut, rather than selecting, checking or evaluating another call.

use checked_trees::{FlowFacts, FlowTailTransferFact};
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

pub(crate) struct JumpTarget<'program> {
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) arguments: &'program [ExpressionHandle],
    pub(crate) tail: Option<&'program FlowTailTransferFact>,
}

pub(crate) fn tail<'program>(
    program: &'program dyn validation::CheckingInput,
    flow: &'program FlowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    transition_target: TransitionTargetHandle,
) -> Option<JumpTarget<'program>> {
    let mut rows = flow.control.tail_transfers.iter().filter_map(|(_, row)| {
        (row.machine_symbol == machine.symbol
            && row.state_symbol == state.symbol
            && row.statement_index == statement_index
            && row.transition_target == transition_target)
            .then_some(row)
    });
    let row = rows.next()?;
    if rows.next().is_some() || !flow.control.calls.is_valid(row.call) {
        return None;
    }
    let source = match program
        .statement_table
        .statement_at(state, statement_index)?
    {
        StatementNode::Expression(expression)
            if !transition_target.is_valid()
                && statement_index + 1
                    == program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len() =>
        {
            *expression
        }
        StatementNode::Transition(transition)
            if transition
                .targets()
                .any(|target| target == transition_target) =>
        {
            let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition_target)
            else {
                return None;
            };
            *expression
        }
        _ => return None,
    };
    let call = flow.control.calls.get(row.call);
    let expression_call = program.expression_table.call_node(source)?;
    let entry = program.machine_states(machine).first()?;
    let mut owners = flow.control.states.iter().filter_map(|(_, owner)| {
        (owner.machine_symbol == machine.symbol && owner.state_symbol == state.symbol)
            .then_some(owner)
    });
    let owner = owners.next()?;
    if owners.next().is_some()
        || !owner.calls.handles().any(|handle| handle == row.call)
        || flow.control.is_retired(state.symbol, call)
        || row.expression != source
        || call.authored_expression != source
        || call.statement_index != statement_index
        || call.target_symbol != entry.symbol
        || row.target_state != entry.symbol
        || row.operands != call.argument_operands()
        || row.operands.count() as usize
            != program
                .expression_table
                .expression_handles(expression_call.arguments)
                .len()
    {
        return None;
    }
    let parameters = program.state_parameters(entry);
    let arguments = program
        .expression_table
        .expression_handles(expression_call.arguments);
    let operands = flow.control.operands.span_or_empty(row.operands);
    if parameters.len() != arguments.len()
        || operands.len() != arguments.len()
        || operands.iter().zip(arguments).any(|(operand, argument)| {
            operand.expression != *argument
                || operand.state_symbol != state.symbol
                || operand.statement_index != statement_index
        })
    {
        return None;
    }
    Some(JumpTarget {
        symbol: row.target_state,
        arguments: program
            .expression_table
            .expression_handles(expression_call.arguments),
        tail: Some(row),
    })
}

pub(crate) fn transition<'program>(
    program: &'program dyn validation::CheckingInput,
    flow: &'program FlowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    target: TransitionTargetHandle,
) -> Option<JumpTarget<'program>> {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => Some(JumpTarget {
            symbol: path.symbol,
            arguments: program.statement_table.expression_handles(*arguments),
            tail: None,
        }),
        TransitionTargetNode::Value(_) => {
            tail(program, flow, machine, state, statement_index, target)
        }
        TransitionTargetNode::Terminal => None,
    }
}
