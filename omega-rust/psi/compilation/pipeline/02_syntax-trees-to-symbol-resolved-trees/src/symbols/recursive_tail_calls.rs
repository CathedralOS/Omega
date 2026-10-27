//! Exact recursive tail calls retain invocation admission on constant-stack edges.
//!
//! Run once after all declaration/conformance/target binding, on authoritative
//! statement nodes. Earlier ordinary resolution can still name a target-family
//! interface, not this body; normalizing then misses legal recursive transfers.
//! Re-running name assignment here would undo final binding. Legacy statement
//! slices are no longer the consumer's input and must not be rebuilt over these
//! nodes. Ranking remains independent of tail position; later checking admits
//! operands, invocation contracts, ownership and exact authored call markers.

use arena::HandleSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::machine::Machine;
use symbol_resolved_trees::statement::{
    StatementNode, TableNamePath, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};
use symbols::SymbolHandle;

use super::scope::{AttachedMachines, attached_machines};

#[derive(Clone, Copy)]
struct RecursiveEntry {
    machine: SymbolHandle,
    entry: SymbolHandle,
    owner: SymbolHandle,
}

pub(crate) fn normalize(program: &mut SymbolResolvedTrees) {
    let attached = attached_machines(program);
    for machine_position in 0..program.machines.len() {
        let machine = &program.machines[machine_position];
        if !unambiguous_canonical_entry(program, &attached, machine) {
            continue;
        }
        let states = machine.states;
        let Some(&entry_handle) = program.machine_state_handles(states).first() else {
            continue;
        };
        let entry = program.machine_state(entry_handle);
        let identity = RecursiveEntry {
            machine: machine.symbol,
            entry: entry.symbol,
            owner: machine.attached_data_symbol,
        };
        let result = entry.return_type.clone();
        for state_position in 0..program.machine_state_handles(states).len() {
            let state_handle = program.machine_state_handles(states)[state_position];
            let state = program.machine_state(state_handle);
            // A jump does not perform the value call's result compatibility
            // check. Require complete resolved declarations to agree, including
            // child handles and constraints; uncertain equality stays a call.
            if state.return_type == result {
                normalize_state(program, identity, state.statement_nodes);
            }
        }
    }
}

fn unambiguous_canonical_entry(
    program: &SymbolResolvedTrees,
    attached: &AttachedMachines,
    machine: &Machine,
) -> bool {
    if !machine.attached_data_symbol.is_valid() {
        return true;
    }
    // Target siblings are deliberately absent from ambient method lookup.
    // Their settled family identifies the canonical callable once; never add
    // one overload per target or collapse unrelated declarations by spelling.
    let canonical_symbol = if machine.target_family.is_valid() {
        machine.target_family
    } else {
        machine.symbol
    };
    let canonical = if canonical_symbol == machine.symbol {
        Some(machine)
    } else {
        program.machine(canonical_symbol)
    };
    let Some(canonical) = canonical else {
        return false;
    };
    let Some(&entry) = program.machine_state_handles(canonical.states).first() else {
        return false;
    };
    let entry_name = program.symbols.name(program.machine_state(entry).symbol);
    let mut candidates = attached
        .positions_for_owner(canonical.attached_data_symbol)
        .iter()
        .map(|&position| &attached.list[position])
        .filter(|candidate| program.symbols.name(candidate.entry) == entry_name)
        .map(|candidate| candidate.machine);
    // A provisional result-overload selection is not exact invocation identity.
    // Zero candidates is not uniqueness either. Leave either case for typing.
    candidates.next() == Some(canonical_symbol) && candidates.next().is_none()
}

fn normalize_state(
    program: &mut SymbolResolvedTrees,
    identity: RecursiveEntry,
    statements: HandleSpan<StatementNode>,
) {
    let final_position = statements.len().checked_sub(1);
    for (position, statement_handle) in statements.handles().enumerate() {
        match program
            .tables
            .bodies
            .statements
            .statement(statement_handle)
            .clone()
        {
            StatementNode::Expression(expression) if Some(position) == final_position => {
                if let Some(target) = recursive_target(program, identity, expression) {
                    let source_span = program.tables.bodies.expressions.source_span(expression);
                    let target = program
                        .tables
                        .bodies
                        .statements
                        .insert_transition_target(target);
                    *program
                        .tables
                        .bodies
                        .statements
                        .statement_mut(statement_handle) =
                        StatementNode::Transition(TableTransition {
                            target,
                            source_span,
                            ..Default::default()
                        });
                }
            }
            StatementNode::Transition(transition) => {
                normalize_target(program, identity, transition.target);
                if transition.continuation.is_valid() {
                    normalize_target(program, identity, transition.continuation);
                }
            }
            _ => {}
        }
    }
}

fn normalize_target(
    program: &mut SymbolResolvedTrees,
    identity: RecursiveEntry,
    target: TransitionTargetHandle,
) {
    let TransitionTargetNode::Value(expression) =
        *program.tables.bodies.statements.transition_target(target)
    else {
        return;
    };
    if let Some(named) = recursive_target(program, identity, expression) {
        *program
            .tables
            .bodies
            .statements
            .transition_target_mut(target) = named;
    }
}

fn recursive_target(
    program: &mut SymbolResolvedTrees,
    identity: RecursiveEntry,
    expression: ExpressionHandle,
) -> Option<TransitionTargetNode> {
    let expressions = &program.tables.bodies.expressions;
    let call = expressions.call_node(expression)?;
    if !identity.entry.is_valid()
        || (call.target_symbol != identity.entry && call.target_symbol != identity.machine)
        || !call.machine_arguments.is_empty()
    {
        return None;
    }
    // A declaration qualifier carries no value. Runtime binding dispatch and
    // calls on another receiver must retain ordinary invocation semantics.
    if call.receiver.is_valid()
        && !matches!(expressions.expression(call.receiver), ExpressionNode::Name(receiver)
            if identity.owner.is_valid() && receiver.symbol == identity.owner)
    {
        return None;
    }
    let mut occurrences = expressions.authored_selection_occurrences(expression);
    let authored_call_selection = occurrences.next();
    if occurrences.next().is_some() {
        // The named target can carry one exact occurrence, not a lossy choice
        // among several independently retained selections.
        return None;
    }
    let statements = &mut program.tables.bodies.statements;
    let arguments = expressions.expression_handles(call.arguments);
    let retained = statements.reserve_expression_handles(u32::try_from(arguments.len()).ok()?);
    for (position, &argument) in arguments.iter().enumerate() {
        statements.set_expression_handle_at_offset(
            retained,
            u32::try_from(position).ok()?,
            argument,
        );
    }
    Some(TransitionTargetNode::Named {
        operational_acknowledgement: call.operational_acknowledgement,
        path: TableNamePath {
            receiver_symbol: Default::default(),
            members: statements.insert_name_path_members(std::slice::from_ref(&call.target)),
            head_symbol: identity.machine,
            symbol: identity.machine,
        },
        arguments: retained,
        evidence_arguments: call.evidence_arguments.clone(),
        source_span: call.target.source_span(),
        authored_call_selection,
    })
}
