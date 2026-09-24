//! Rewriting cloned calls and static machine transition targets.

use crate::monomorphization::body_rewriting::evidence_rewrites::{
    evidence_argument_rewrites, evidence_requirement_rewrites,
};
use crate::monomorphization::body_rewriting::machine_arguments::{
    forwarded_static_argument_rewrites, substitute_forwarded_machine_arguments,
};
use crate::monomorphization::body_rewriting::{
    cloned_runtime_call_subjects, insert_subject_name, runtime_value_subjects,
};
use crate::monomorphization::selection::state_by_symbol;
use crate::monomorphization::{
    Candidate, ExpressionHandle, ExpressionNode, Handle, HandleSpan, StatementNode, SymbolHandle,
    TypedTrees,
};

pub(crate) fn span_without_first<T>(span: HandleSpan<T>) -> HandleSpan<T> {
    if span.count() <= 1 {
        return HandleSpan::empty();
    }
    let start = span.start();
    HandleSpan::from_parts(
        Handle::from_parts(
            start
                .arena_index()
                .checked_add(1)
                .expect("argument span index overflow"),
            start.generation(),
        ),
        span.count() - 1,
    )
}

pub(crate) fn statement_span_handles(
    span: HandleSpan<StatementNode>,
) -> Vec<Handle<StatementNode>> {
    (0..span.count())
        .map(|offset| {
            Handle::from_parts(
                span.start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("statement span index overflow"),
                span.start().generation(),
            )
        })
        .collect()
}

pub(crate) fn statement_receiver_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(
    SymbolHandle,
    SymbolHandle,
    Vec<typed_trees::name::Identifier>,
)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => statement_receiver_path(program, atomic.value),
        ExpressionNode::Borrow(inner) => statement_receiver_path(program, inner.target),
        ExpressionNode::Name(path) => Some((
            path.head_symbol,
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .to_vec(),
        )),
        ExpressionNode::Member(member) => {
            let (root, symbol, mut path) = statement_receiver_path(program, member.receiver)?;
            path.push(member.member.clone());
            Some((root, symbol, path))
        }
        _ => None,
    }
}

pub(crate) fn rewrite_cloned_calls(
    source: Option<&TypedTrees>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    state_transition_subjects: &[Vec<(typed_trees::name::Identifier, SymbolHandle)>],
    expression_start: usize,
    states: HandleSpan<typed_trees::state::State>,
) {
    let machine_rewrites: Vec<_> = candidate
        .template
        .machine_parameters
        .iter()
        .zip(candidate.machine_bindings.iter())
        .map(|((parameter, _, _), binding)| {
            let binding = binding.as_ref().expect("complete specialization");
            let name = binding
                .path
                .last()
                .cloned()
                .or_else(|| {
                    state_by_symbol(source.unwrap_or(program), binding.symbol)
                        .map(|state| state.name.clone())
                })
                .expect("static machine entry name");
            (*parameter, binding.symbol, name)
        })
        .collect();
    let evidence_target_rewrites =
        evidence_requirement_rewrites(source.unwrap_or(program), candidate);
    let mut target_rewrites = machine_rewrites.clone();
    target_rewrites.extend(
        evidence_target_rewrites
            .iter()
            .map(|rewrite| (rewrite.placeholder, rewrite.target, rewrite.name.clone())),
    );
    let mut argument_rewrites = machine_rewrites;
    argument_rewrites.extend(evidence_argument_rewrites(
        source.unwrap_or(program),
        candidate,
    ));
    let static_argument_rewrites =
        forwarded_static_argument_rewrites(source.unwrap_or(program), candidate);
    for (state_index, state) in program
        .machine_states
        .span_or_empty(states)
        .to_vec()
        .into_iter()
        .enumerate()
    {
        for statement_handle in statement_span_handles(state.statement_nodes) {
            rewrite_static_machine_transition_targets(
                program,
                statement_handle,
                candidate,
                &target_rewrites,
            );
            rewrite_clone_transition_subjects(
                program,
                statement_handle,
                state_transition_subjects
                    .get(state_index)
                    .map_or(&[], Vec::as_slice),
                state_symbols,
            );
            let StatementNode::Call(snapshot) =
                program.statement_table.statement(statement_handle).clone()
            else {
                continue;
            };
            // A call selecting a sibling specialization state forwards each
            // runtime-bound `Value` subject as an ordinary argument.
            let clone_state_arguments = if state_symbols
                .iter()
                .any(|(_, concrete)| *concrete == snapshot.target_symbol)
            {
                let subjects = runtime_value_subjects(program, &state, &snapshot.machine_arguments);
                (!subjects.is_empty()).then(|| {
                    let mut arguments = program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .to_vec();
                    for (member, symbol) in subjects.iter().cloned() {
                        arguments.push(insert_subject_name(
                            &mut program.expression_table,
                            member,
                            symbol,
                        ));
                    }
                    program.statement_table.insert_expression_handles(arguments)
                })
            } else {
                None
            };
            let evidence_dispatch = evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == snapshot.target_symbol);
            let evidence_receiver = evidence_dispatch
                .is_some()
                .then(|| {
                    program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .first()
                        .copied()
                })
                .flatten()
                .and_then(|receiver| statement_receiver_path(program, receiver));
            let receiver = evidence_receiver.as_ref().map(|(_, _, members)| {
                let mut receiver = HandleSpan::empty();
                for member in members {
                    program
                        .statement_table
                        .push_name_path_member(&mut receiver, member.clone());
                }
                receiver
            });
            let StatementNode::Call(call) = program.statement_table.statement_mut(statement_handle)
            else {
                unreachable!();
            };
            if candidate
                .template
                .machine_parameters
                .iter()
                .any(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.static_machine_parameter = call.target_symbol;
            }
            if let Some((_, target, name)) = target_rewrites
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
            {
                call.target_symbol = *target;
                call.target = name.clone();
            }
            if let (Some((root, receiver_symbol, _)), Some(receiver)) =
                (evidence_receiver, receiver)
            {
                call.receiver_root_symbol = root;
                call.receiver_symbol = receiver_symbol;
                call.receiver = receiver;
                call.arguments = span_without_first(call.arguments);
            } else if evidence_dispatch.is_some() {
                call.receiver_root_symbol = SymbolHandle::invalid();
                call.receiver_symbol = SymbolHandle::invalid();
                call.receiver = HandleSpan::empty();
            }
            if let Some(rewrite) = evidence_dispatch {
                call.static_requirement_dispatch = Some(rewrite.dispatch.clone());
                call.machine_arguments = rewrite.application_arguments.clone();
            }
            substitute_forwarded_machine_arguments(
                &mut call.machine_arguments,
                &static_argument_rewrites,
                &argument_rewrites,
            );
            if state_symbols
                .iter()
                .any(|(_, concrete)| *concrete == call.target_symbol)
            {
                if let Some(arguments) = clone_state_arguments {
                    call.arguments = arguments;
                }
                call.machine_arguments = Box::default();
            }
        }
    }
    let runtime_calls =
        cloned_runtime_call_subjects(program, states, state_symbols, expression_start);
    let handles: Vec<_> = program
        .expression_table
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() as usize >= expression_start)
        .map(|(handle, _)| handle)
        .collect();
    for handle in handles {
        let clone_state_arguments = match program.expression_table.expression(handle).clone() {
            ExpressionNode::Call(snapshot)
                if state_symbols
                    .iter()
                    .any(|(_, concrete)| *concrete == snapshot.target_symbol) =>
            {
                let subjects = runtime_calls
                    .binary_search_by_key(
                        &(handle.arena_index(), handle.generation()),
                        |(expression, _)| (expression.arena_index(), expression.generation()),
                    )
                    .ok()
                    .map(|ordinal| runtime_calls[ordinal].1.as_slice())
                    .unwrap_or(&[]);
                (!subjects.is_empty()).then(|| {
                    let mut arguments = program
                        .expression_table
                        .expression_handles(snapshot.arguments)
                        .to_vec();
                    for (member, symbol) in subjects.iter().cloned() {
                        arguments.push(insert_subject_name(
                            &mut program.expression_table,
                            member,
                            symbol,
                        ));
                    }
                    program
                        .expression_table
                        .insert_expression_handles(arguments)
                })
            }
            _ => None,
        };
        let evidence_dispatch = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == call.target_symbol),
            _ => None,
        };
        let evidence_receiver = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) if evidence_dispatch.is_some() => program
                .expression_table
                .expression_handles(call.arguments)
                .first()
                .copied(),
            _ => None,
        };
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        if candidate
            .template
            .machine_parameters
            .iter()
            .any(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.static_machine_parameter = call.target_symbol;
        }
        if let Some((_, target, name)) = target_rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *target;
            call.target = name.clone();
        }
        if evidence_dispatch.is_some()
            && let Some(receiver) = evidence_receiver
        {
            call.receiver = receiver;
            call.arguments = span_without_first(call.arguments);
        } else if evidence_dispatch.is_some() {
            call.receiver = ExpressionHandle::invalid();
        }
        if let Some(rewrite) = evidence_dispatch {
            call.static_requirement_dispatch = Some(rewrite.dispatch.clone());
            call.machine_arguments = rewrite.application_arguments.clone();
        }
        substitute_forwarded_machine_arguments(
            &mut call.machine_arguments,
            &static_argument_rewrites,
            &argument_rewrites,
        );
        if state_symbols
            .iter()
            .any(|(_, concrete)| *concrete == call.target_symbol)
        {
            if let Some(arguments) = clone_state_arguments {
                call.arguments = arguments;
            }
            call.machine_arguments = Box::default();
        }
    }
}

pub(crate) fn rewrite_static_machine_transition_targets(
    program: &mut TypedTrees,
    statement: typed_trees::statement::StatementHandle,
    candidate: &Candidate,
    rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
) {
    let StatementNode::Transition(transition) = program.statement_table.statement(statement) else {
        return;
    };
    let targets = [transition.target, transition.continuation];
    for target in targets {
        if !target.is_valid() {
            continue;
        }
        let typed_trees::statement::TransitionTargetNode::Named {
            path,
            static_machine_parameter,
            ..
        } = program.statement_table.transition_target_mut(target)
        else {
            continue;
        };
        if !candidate
            .template
            .machine_parameters
            .iter()
            .any(|(parameter, _, _)| *parameter == path.symbol)
        {
            continue;
        }
        if let Some((parameter, selected, _)) = rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == path.symbol)
        {
            // Retain the authored binder separately from executable selection,
            // just as expression and statement calls do. Internal state
            // transfers never enter this exact machine-parameter rewrite.
            *static_machine_parameter = *parameter;
            path.symbol = *selected;
            if path.head_symbol == *parameter {
                path.head_symbol = *selected;
            }
        }
    }
}

/// Every cloned state carries the runtime-bound `Value` subjects as trailing
/// ordinary parameters, so a transition between specialization states owes
/// the target its subject arguments exactly as a rewritten call site does.
/// Append the containing state's realized parameters in telescope order.
pub(crate) fn rewrite_clone_transition_subjects(
    program: &mut TypedTrees,
    statement: typed_trees::statement::StatementHandle,
    subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
    state_symbols: &[(SymbolHandle, SymbolHandle)],
) {
    if subjects.is_empty() {
        return;
    }
    let StatementNode::Transition(transition) = program.statement_table.statement(statement) else {
        return;
    };
    let targets = [transition.target, transition.continuation];
    for target in targets {
        if !target.is_valid() {
            continue;
        }
        let is_clone_state = matches!(
            program.statement_table.transition_target(target),
            typed_trees::statement::TransitionTargetNode::Named { path, .. }
                if state_symbols
                    .iter()
                    .any(|(_, concrete)| *concrete == path.symbol)
        );
        if !is_clone_state {
            continue;
        }
        let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target(target)
        else {
            unreachable!("named transition target checked above");
        };
        let mut arguments = program
            .statement_table
            .expression_handles(*arguments)
            .to_vec();
        for (member, symbol) in subjects.iter().cloned() {
            arguments.push(insert_subject_name(
                &mut program.expression_table,
                member,
                symbol,
            ));
        }
        let new_arguments = program.statement_table.insert_expression_handles(arguments);
        let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target_mut(target)
        else {
            unreachable!("named transition target checked above");
        };
        *arguments = new_arguments;
    }
}
