//! Read-only state-transfer topology queries for write-frame inference.
//!
//! This leaf resolves authored state edges and checked whole-call tails within
//! one machine, checks their reachable subgraph for cycles, and recognizes exact
//! write-capable namespace preservation for contextual prefixes. It does not
//! build or solve frame equations; complete cycle frames use root substitutions.

use super::state_paths::expression_forwards_exact_symbol;
use super::type_capabilities::parameter_may_carry_write;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

/// Borrowed source-state transfer, independent of either argument-span arena.
/// The caller still owns application, loan and ownership admission.
pub(super) struct StateTransfer<'program> {
    pub(super) target: &'program State,
    pub(super) arguments: &'program [ExpressionHandle],
}

pub(super) fn expression_state_transfer<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    source: &'program State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<StateTransfer<'program>> {
    let call =
        crate::current_entry_tail_call(program, machine, source, statement_index, expression)?;
    Some(StateTransfer {
        target: program.machine_states(machine).first()?,
        arguments: program.expression_table.expression_handles(call.arguments),
    })
}

pub(super) fn transition_state_transfer<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    source: &'program State,
    statement_index: usize,
    target: TransitionTargetHandle,
) -> Option<StateTransfer<'program>> {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. }
            if !super::terminal_calls::target(program, machine, target).is_valid() =>
        {
            Some(StateTransfer {
                target: named_transition_target_state(program, machine, source, target)?,
                arguments: program.statement_table.expression_handles(*arguments),
            })
        }
        TransitionTargetNode::Value(expression) => {
            expression_state_transfer(program, machine, source, statement_index, *expression)
        }
        _ => None,
    }
}

pub(super) fn statement_state_transfers<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    source: &'program State,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<Vec<StateTransfer<'program>>> {
    let mut transfers = Vec::new();
    match statement {
        StatementNode::Expression(expression) => {
            transfers.extend(expression_state_transfer(
                program,
                machine,
                source,
                statement_index,
                *expression,
            ));
        }
        StatementNode::Transition(transition) => {
            for target in transition.targets() {
                if super::terminal_calls::target(program, machine, target).is_valid() {
                    continue;
                }
                let transfer =
                    transition_state_transfer(program, machine, source, statement_index, target);
                if matches!(
                    program.statement_table.transition_target(target),
                    TransitionTargetNode::Named { .. }
                ) && transfer.is_none()
                {
                    return None;
                }
                transfers.extend(transfer);
            }
        }
        _ => {}
    }
    Some(transfers)
}

/// A transfer's operands execute before the source scope closes. Its target
/// body is composed through the state equations, never as a returning call.
pub(super) fn frame_operand_expression_roots(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source: &State,
    statement_index: usize,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    let mut operands = Vec::new();
    for expression in super::demand::statement_value_expression_roots(program, statement) {
        if let Some(call) =
            crate::current_entry_tail_call(program, machine, source, statement_index, expression)
        {
            if call.receiver.is_valid() {
                operands.push(call.receiver);
            }
            operands.extend(program.expression_table.expression_handles(call.arguments));
        } else {
            operands.push(expression);
        }
    }
    operands
}

fn transfer_subgraph_is_acyclic(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    visiting: &mut Vec<SymbolHandle>,
    complete: &mut Vec<SymbolHandle>,
) -> bool {
    if complete.contains(&state.symbol) {
        return true;
    }
    if visiting.contains(&state.symbol) {
        return false;
    }
    visiting.push(state.symbol);
    for (index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let Some(transfers) = statement_state_transfers(program, machine, state, index, statement)
        else {
            return false;
        };
        for transfer in transfers {
            if !transfer_subgraph_is_acyclic(program, machine, transfer.target, visiting, complete)
            {
                return false;
            }
        }
    }
    visiting.pop();
    complete.push(state.symbol);
    true
}

pub(super) fn transition_transfer_subgraph_is_acyclic(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source: &State,
    target: TransitionTargetHandle,
) -> bool {
    if super::terminal_calls::target(program, machine, target).is_valid() {
        return true;
    }
    let Some(index) = program
        .statement_table
        .indexed_statements(source.statement_nodes)
        .find_map(|(index, statement)| match statement {
            StatementNode::Transition(transition)
                if transition.targets().any(|candidate| candidate == target) =>
            {
                Some(index)
            }
            _ => None,
        })
    else {
        return false;
    };
    let Some(transfer) = transition_state_transfer(program, machine, source, index, target) else {
        return false;
    };
    transfer_subgraph_is_acyclic(
        program,
        machine,
        transfer.target,
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

/// Complete topology, including preserved whole-call tails, determines whether
/// a cached summary can depend on a still-active ancestor's provisional frame.
pub(crate) fn state_transfer_subgraph_is_acyclic(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
) -> bool {
    transfer_subgraph_is_acyclic(program, machine, state, &mut Vec::new(), &mut Vec::new())
}

pub(super) fn named_transition_target_state<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    source: &'program State,
    target: TransitionTargetHandle,
) -> Option<&'program State> {
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return None;
    };
    // A tail call to the machine itself re-enters its entry state.
    if path.symbol == machine.symbol {
        return program.machine_states(machine).first();
    }
    program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.symbol == path.symbol)
        .or_else(|| {
            let members = program.statement_table.name_path_members(path.members);
            matches!(members, [member] if member.is_self_receiver()).then_some(source)
        })
}

/// Tarjan's algorithm over successor lists; an edge lies on a cycle exactly
/// when both ends share a component. A self edge keeps its node in the same
/// component, which matches the solver's reachability test.
pub(super) fn strongly_connected_components(successors: &[Vec<usize>]) -> Vec<usize> {
    struct Search<'graph> {
        successors: &'graph [Vec<usize>],
        index: Vec<Option<usize>>,
        lowlink: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        component: Vec<usize>,
        next_index: usize,
        next_component: usize,
    }

    impl Search<'_> {
        fn visit(&mut self, node: usize) {
            self.index[node] = Some(self.next_index);
            self.lowlink[node] = self.next_index;
            self.next_index += 1;
            self.stack.push(node);
            self.on_stack[node] = true;
            for position in 0..self.successors[node].len() {
                let target = self.successors[node][position];
                match self.index[target] {
                    None => {
                        self.visit(target);
                        self.lowlink[node] = self.lowlink[node].min(self.lowlink[target]);
                    }
                    Some(target_index) if self.on_stack[target] => {
                        self.lowlink[node] = self.lowlink[node].min(target_index);
                    }
                    Some(_) => {}
                }
            }
            if self.lowlink[node] == self.index[node].unwrap_or(usize::MAX) {
                while let Some(member) = self.stack.pop() {
                    self.on_stack[member] = false;
                    self.component[member] = self.next_component;
                    if member == node {
                        break;
                    }
                }
                self.next_component += 1;
            }
        }
    }

    let count = successors.len();
    let mut search = Search {
        successors,
        index: vec![None; count],
        lowlink: vec![0; count],
        on_stack: vec![false; count],
        stack: Vec::new(),
        component: vec![0; count],
        next_index: 0,
        next_component: 0,
    };
    for node in 0..count {
        if search.index[node].is_none() {
            search.visit(node);
        }
    }
    search.component
}

/// A state transfer closing a cycle is frame-equivalent to a bare `self`
/// edge when every parameter capable of carrying caller-visible writes is fed
/// by the source parameter at that same ordinal. Reordering primitive values
/// and shared references cannot redirect a write and therefore does not make
/// an otherwise finite frame opaque. Parameter symbols are state-local, so a
/// multi-state cycle compares each write-capable argument to the source
/// namespace rather than requiring the target's distinct symbol.
pub(super) fn transfer_preserves_state_namespace(
    program: &dyn crate::CheckingInput,
    source_state: &State,
    target_state: &State,
    arguments: &[ExpressionHandle],
) -> bool {
    let source_parameters = program
        .state_parameters(source_state)
        .iter()
        .collect::<Vec<_>>();
    let target_parameters = program
        .state_parameters(target_state)
        .iter()
        .collect::<Vec<_>>();
    source_parameters.len() == target_parameters.len()
        && target_parameters.len() == arguments.len()
        && source_parameters
            .into_iter()
            .zip(target_parameters)
            .zip(arguments.iter().copied())
            .all(|((source, target), argument)| {
                !(parameter_may_carry_write(program, source)
                    || parameter_may_carry_write(program, target))
                    || expression_forwards_exact_symbol(program, argument, source.symbol)
            })
}
