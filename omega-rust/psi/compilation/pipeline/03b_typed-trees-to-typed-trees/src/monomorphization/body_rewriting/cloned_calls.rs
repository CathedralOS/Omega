//! Rewriting cloned calls and static machine transition targets.

use crate::monomorphization::application_arguments::substitute_forwarded_machine_arguments;
use crate::monomorphization::application_calls::{
    ApplicationCallRewrites, apply_evidence_dispatch_rewrite,
};
use crate::monomorphization::body_rewriting::{
    cloned_runtime_call_subjects, insert_subject_names, runtime_value_subjects,
};
use crate::monomorphization::{
    Candidate, ExpressionNode, Handle, HandleSpan, StatementNode, SymbolHandle, TypedTrees,
};

pub(crate) fn statement_span_handles(
    span: HandleSpan<StatementNode>,
) -> Vec<Handle<StatementNode>> {
    span.handles().collect()
}

pub(crate) fn rewrite_cloned_calls(
    source: Option<&dyn ::validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    state_transition_subjects: &[Vec<(typed_trees::name::Identifier, SymbolHandle)>],
    expression_start: usize,
    states: HandleSpan<typed_trees::state::State>,
) -> Result<(), diagnostics::Diagnostic> {
    let application_bindings = crate::monomorphization::applications::candidate_bindings(candidate);
    let rewrites = ApplicationCallRewrites::prepare(
        source.unwrap_or(program),
        &application_bindings,
        &candidate.template.evidence_parameters,
    )?;
    let ApplicationCallRewrites {
        target_rewrites,
        selected_carriers,
        evidence_target_rewrites,
        argument_rewrites,
        static_argument_rewrites,
        ..
    } = &rewrites;
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
                    super::insert_arguments_with_subjects(program, snapshot.arguments, &subjects)
                })
            } else {
                None
            };
            let evidence_dispatch = evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == snapshot.target_symbol);
            let selected_carrier = selected_carriers
                .iter()
                .find(|(parameter, _, _)| *parameter == snapshot.target_symbol)
                .map(|(_, symbol, name)| {
                    let mut members = HandleSpan::empty();
                    program
                        .statement_table
                        .push_name_path_member(&mut members, name.clone());
                    (*symbol, members)
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
            if let Some((symbol, members)) = selected_carrier {
                call.receiver_root_symbol = symbol;
                call.receiver_symbol = symbol;
                call.receiver = members;
            }
            // Static requirement dispatch selects a concrete declaration; the
            // conformance-evidence namespace that spelled the call is not a
            // runtime receiver and the rewritten target owns every parameter as
            // an explicit argument. Clearing the receiver leaves the operand
            // roster exactly as authored.
            let erased_application = apply_evidence_dispatch_rewrite(
                call.target_symbol,
                &mut call.static_requirement_dispatch,
                &mut call.machine_arguments,
                &mut call.arguments,
                evidence_dispatch,
                &static_argument_rewrites,
                &argument_rewrites,
                state_symbols,
                clone_state_arguments,
                || {
                    call.receiver_root_symbol = SymbolHandle::invalid();
                    call.receiver_symbol = SymbolHandle::invalid();
                    call.receiver = HandleSpan::empty();
                },
            );
            let application = call.authored_machine_application;
            let target = call.target.clone();
            if application.is_valid() {
                substitute_forwarded_machine_arguments(
                    program
                        .expression_table
                        .static_machine_application_arguments_mut(application),
                    &static_argument_rewrites,
                    &argument_rewrites,
                );
                substitute_forwarded_machine_arguments(
                    program
                        .expression_table
                        .static_machine_application_selected_machines_mut(application),
                    &static_argument_rewrites,
                    &argument_rewrites,
                );
            } else if let Some((registration_operation, arguments)) = erased_application {
                let application = program.expression_table.retain_static_machine_application(
                    registration_operation,
                    target,
                    &arguments,
                );
                let StatementNode::Call(call) =
                    program.statement_table.statement_mut(statement_handle)
                else {
                    unreachable!();
                };
                call.authored_machine_application = application;
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
                    insert_subject_names(&mut program.expression_table, &mut arguments, subjects);
                    program
                        .expression_table
                        .insert_expression_handles(arguments)
                })
            }
            _ => None,
        };
        rewrites.rewrite_expression(program, handle, state_symbols, clone_state_arguments);
    }
    Ok(())
}

pub(crate) fn rewrite_static_machine_transition_targets(
    program: &mut TypedTrees,
    statement: typed_trees::statement::StatementHandle,
    candidate: &Candidate,
    rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
) {
    let Some(transition) = program.statement_table.statement(statement).as_transition() else {
        return;
    };
    for target in [transition.target, transition.continuation]
        .into_iter()
        .filter(|handle| handle.is_valid())
    {
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
    let Some(transition) = program.statement_table.statement(statement).as_transition() else {
        return;
    };
    for target in [transition.target, transition.continuation]
        .into_iter()
        .filter(|handle| handle.is_valid())
    {
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
        insert_subject_names(&mut program.expression_table, &mut arguments, subjects);
        let new_arguments = program.statement_table.insert_expression_handles(arguments);
        let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
            program.statement_table.transition_target_mut(target)
        else {
            unreachable!("named transition target checked above");
        };
        *arguments = new_arguments;
    }
}
