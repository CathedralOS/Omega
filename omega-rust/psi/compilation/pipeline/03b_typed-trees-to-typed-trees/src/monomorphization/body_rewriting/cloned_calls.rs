//! Rewriting cloned calls and static machine transition targets.

use crate::monomorphization::body_rewriting::evidence_rewrites::{
    evidence_argument_rewrites, evidence_requirement_rewrites, rewrite_index_algebra,
};
use crate::monomorphization::body_rewriting::machine_arguments::{
    forwarded_static_argument_rewrites, substitute_forwarded_machine_arguments,
};
use crate::monomorphization::body_rewriting::{
    cloned_runtime_call_subjects, insert_subject_name, insert_subject_names,
    insert_subject_operands, runtime_value_operands,
};
use crate::monomorphization::selection::state_by_symbol;
use crate::monomorphization::{
    Candidate, ExpressionHandle, ExpressionNode, Handle, HandleSpan, StatementNode, SymbolHandle,
    TypedTrees,
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
    // Selection changes declaration identity, not the explicit operand roster.
    // An attached declaration retains its Data namespace so ordinary call
    // validation cannot reinterpret the first operand as an implicit receiver.
    let selected_carriers: Vec<_> = machine_rewrites
        .iter()
        .filter_map(|(parameter, target, _)| {
            let declarations = source.unwrap_or(program);
            let machine = declarations.machines().iter().find(|machine| {
                typed_trees::state::state_by_symbol(declarations.machine_states(machine), *target)
                    .is_some()
            })?;
            if !machine.attached_data_symbol.is_valid() {
                return None;
            }
            Some((
                *parameter,
                machine.attached_data_symbol,
                machine.attached_data.as_ref()?.clone(),
            ))
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
    let evidence_rewrites = evidence_argument_rewrites(source.unwrap_or(program), candidate);
    argument_rewrites.extend(evidence_rewrites.iter().map(|rewrite| {
        (
            rewrite.binder,
            rewrite.argument.symbol,
            rewrite.alias.clone(),
        )
    }));
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
                let subjects =
                    runtime_value_operands(program, state.symbol, &snapshot.machine_arguments);
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
        rewrite_index_algebra(
            program,
            handle,
            &evidence_rewrites,
            &static_argument_rewrites,
        );
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
                    insert_subject_operands(&mut arguments, subjects);
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
        let selected_carrier = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => selected_carriers
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
                .map(|(_, symbol, name)| (*symbol, name.clone())),
            _ => None,
        }
        .map(|(symbol, name)| insert_subject_name(&mut program.expression_table, name, symbol));
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
        if let Some(receiver) = selected_carrier {
            call.receiver = receiver;
        }
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
            || call.receiver = ExpressionHandle::invalid(),
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
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                unreachable!();
            };
            call.authored_machine_application = application;
        }
    }
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

/// The shared tail of an evidence-dispatched call rewrite: `invalidate_receiver`
/// clears the per-node receiver shape, the selected dispatch and its
/// application arguments replace the placeholder's contract, forwarded statics
/// substitute, and a call into the specialization's own states demotes its
/// machine arguments in favor of cloned runtime arguments.
#[allow(clippy::too_many_arguments)]
fn apply_evidence_dispatch_rewrite(
    target_symbol: SymbolHandle,
    static_requirement_dispatch: &mut Option<typed_trees::typed_trees::StaticRequirementDispatch>,
    machine_arguments: &mut Box<[typed_trees::expression::StaticMachineArgument]>,
    arguments: &mut HandleSpan<ExpressionHandle>,
    evidence_dispatch: Option<&super::evidence_rewrites::EvidenceRequirementRewrite>,
    static_argument_rewrites: &[(SymbolHandle, typed_trees::expression::StaticMachineArgument)],
    argument_rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    clone_state_arguments: Option<HandleSpan<ExpressionHandle>>,
    mut invalidate_receiver: impl FnMut(),
) -> Option<(
    SymbolHandle,
    Box<[typed_trees::expression::StaticMachineArgument]>,
)> {
    if evidence_dispatch.is_some() {
        invalidate_receiver();
    }
    if let Some(rewrite) = evidence_dispatch {
        *static_requirement_dispatch = Some(rewrite.dispatch.clone());
        // Map captures belong to the selected conformance application. The
        // method's own invocation operands retain their independent telescope.
    }
    substitute_forwarded_machine_arguments(
        machine_arguments,
        static_argument_rewrites,
        argument_rewrites,
    );
    if let Some((authored, _)) = state_symbols
        .iter()
        .find(|(_, concrete)| *concrete == target_symbol)
    {
        if let Some(cloned) = clone_state_arguments {
            *arguments = cloned;
        }
        return Some((*authored, std::mem::take(machine_arguments)));
    }
    None
}
