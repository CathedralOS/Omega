//! Read-only named-transition topology queries for write-frame inference.
//!
//! This leaf resolves authored named edges within one machine, checks the
//! reachable named-transition subgraph for cycles, recognizes exact
//! write-capable namespace preservation for contextual prefixes. It does not
//! build or solve frame equations; complete cycle frames use root substitutions.

use super::state_paths::expression_forwards_exact_symbol;
use super::type_capabilities::parameter_may_carry_write;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

pub(super) fn named_transition_subgraph_is_acyclic(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: TransitionTargetHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
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
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for edge in [transition.target, transition.continuation] {
                if !edge.is_valid()
                    || super::terminal_calls::target(program, machine, edge).is_valid()
                    || !matches!(
                        program.statement_table.transition_target(edge),
                        TransitionTargetNode::Named { .. }
                    )
                {
                    continue;
                }
                let Some(next) = named_transition_target_state(program, machine, state, edge)
                else {
                    return false;
                };
                if !visit(program, machine, next, visiting, complete) {
                    return false;
                }
            }
        }
        visiting.pop();
        complete.push(state.symbol);
        true
    }

    if super::terminal_calls::target(program, machine, target).is_valid() {
        return true;
    }
    let Some(target) = named_transition_target_state(program, machine, source, target) else {
        return false;
    };
    visit(program, machine, target, &mut Vec::new(), &mut Vec::new())
}

/// True when every named edge leaving `state` lands in an acyclic named
/// subgraph. A state that can reach a named cycle may observe a back-edge to
/// a still-active ancestor during a depth-first walk; the resulting summary
/// depends on the recursion stack that produced it and is valid only for the
/// query that computed it.
pub(crate) fn named_state_transition_subgraph_is_acyclic(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .all(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return true;
            };
            [transition.target, transition.continuation]
                .into_iter()
                .filter(|edge| edge.is_valid())
                .all(|edge| {
                    !matches!(
                        program.statement_table.transition_target(edge),
                        TransitionTargetNode::Named { .. }
                    ) || named_transition_subgraph_is_acyclic(program, machine, state, edge)
                })
        })
}

pub(super) fn named_transition_target_state<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    source: &'program State,
    target: TransitionTargetHandle,
) -> Option<&'program State> {
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return None;
    };
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

/// A named edge closing a state cycle is frame-equivalent to a bare `self`
/// edge when every parameter capable of carrying caller-visible writes is fed
/// by the source parameter at that same ordinal. Reordering primitive values
/// and shared references cannot redirect a write and therefore does not make
/// an otherwise finite frame opaque. Parameter symbols are state-local, so a
/// multi-state cycle compares each write-capable argument to the source
/// namespace rather than requiring the target's distinct symbol.
pub(super) fn named_transition_preserves_state_namespace(
    program: &TypedTrees,
    source_state: &State,
    target_state: &State,
    arguments: &[ExpressionHandle],
) -> bool {
    let source_parameters = program
        .state_parameters(source_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let target_parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
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
