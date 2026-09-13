//! Read-only named-transition topology queries for write-frame inference.
//!
//! This leaf resolves authored named edges within one machine, checks the
//! reachable named-transition subgraph for cycles, and recognizes exact
//! write-capable namespace preservation. It does not build or solve frame
//! equations.

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
pub(super) fn named_state_transition_subgraph_is_acyclic(
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
            matches!(members, [member] if member.as_str() == "self").then_some(source)
        })
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
