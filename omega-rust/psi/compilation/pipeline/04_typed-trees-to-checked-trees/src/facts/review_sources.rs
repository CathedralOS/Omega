//! Attach authored occurrence custody to the checked call topology. Targets and
//! operational acknowledgements come from the operational plan; generated
//! calls acquire their acknowledgements during effect inference, so their
//! checked rows must not be compared with pre-inference syntax here.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionOccurrenceId as OccurrenceId,
};
use source::SourceSpan;
use validation::{CallSite, collect_call_sites_in_statement};

use crate::semantic::calls::dispatch_subjects;
use checked_trees::{FlowCallResult, FlowCallSite};
use typed_trees::statement::StatementNode;

pub(crate) fn bind_checked_body_call_source_spans(
    program: &dyn ::validation::CheckingInput,
    flow: &mut checked_trees::FlowFacts,
) -> Result<(), Vec<Diagnostic>> {
    let machines_by_symbol = program
        .machines()
        .iter()
        .map(|machine| (machine.symbol, machine))
        .collect::<std::collections::HashMap<_, _>>();
    let states_by_symbol = program
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(move |state| ((machine.symbol, state.symbol), state))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let checked_states = flow
        .control
        .states
        .iter()
        .map(|(_, state)| state.clone())
        .collect::<Vec<_>>();
    for checked_state in checked_states {
        let Some(machine) = machines_by_symbol
            .get(&checked_state.machine_symbol)
            .copied()
        else {
            return Err(vec![Diagnostic::error(format!(
                "checked machine {:?} is absent while binding call source custody",
                checked_state.machine_symbol,
            ))]);
        };
        let Some(state) = states_by_symbol
            .get(&(checked_state.machine_symbol, checked_state.state_symbol))
            .copied()
        else {
            return Err(vec![Diagnostic::error(format!(
                "checked state {:?} is absent from machine {:?} while binding call source custody",
                checked_state.state_symbol, checked_state.machine_symbol,
            ))]);
        };
        let mut call_sites_by_statement = std::collections::HashMap::<
            usize,
            (
                Vec<validation::CallSite<'_>>,
                dispatch_subjects::StatementDispatchCalls,
            ),
        >::new();
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut selected_by_statement =
            std::collections::HashMap::<usize, SelectedEvaluation>::new();
        let control = &mut flow.control;
        for checked_call in control.calls.span_mut_or_empty(checked_state.calls) {
            let (call_sites, dispatch_calls) = call_sites_by_statement
                .entry(checked_call.statement_index)
                .or_insert_with(|| {
                    (
                        collect_call_sites_in_statement(
                            program,
                            machine,
                            state,
                            checked_call.statement_index,
                        ),
                        dispatch_subjects::statement_dispatch_calls(
                            program,
                            statements,
                            checked_call.statement_index,
                        ),
                    )
                });
            let Some(call_site) = call_sites.get(checked_call.call_ordinal).copied() else {
                return Err(vec![Diagnostic::error(format!(
                    "checked body call {} in statement {} of machine {:?} state {:?} has no exact typed call site",
                    checked_call.call_ordinal,
                    checked_call.statement_index,
                    checked_state.machine_symbol,
                    checked_state.state_symbol,
                ))]);
            };
            checked_call.authored_expression = match &call_site {
                CallSite::Expression { expression, .. } => *expression,
                CallSite::Statement(_) | CallSite::TransitionNamed { .. } => Default::default(),
            };
            // Lowering emits the invocation from this row: it names the
            // spelling, the callee as spelled, and the explicit arguments, so
            // no later stage walks the statement to find the occurrence again.
            let (site, target, arguments) = match &call_site {
                CallSite::Statement(call) => (
                    FlowCallSite::Statement,
                    call.target_symbol,
                    program.statement_table.expression_handles(call.arguments),
                ),
                CallSite::Expression { call, .. } => (
                    FlowCallSite::Expression,
                    validation::call_target_symbol(program, call),
                    program.expression_table.expression_handles(call.arguments),
                ),
                CallSite::TransitionNamed {
                    path, arguments, ..
                } => (
                    FlowCallSite::NamedTransition,
                    path.symbol,
                    program.statement_table.expression_handles(*arguments),
                ),
            };
            checked_call.authored_receiver = match &call_site {
                CallSite::Expression { call, .. } => call.receiver,
                CallSite::Statement(_) | CallSite::TransitionNamed { .. } => Default::default(),
            };
            checked_call.authored_result = match (
                program
                    .statement_table
                    .statement_at(state, checked_call.statement_index),
                &call_site,
            ) {
                (Some(StatementNode::Call(call)), _)
                    if checked_call.call_ordinal == 0 && call.discards_result =>
                {
                    FlowCallResult::Discarded
                }
                (
                    Some(StatementNode::LocalData(local)),
                    CallSite::Expression { expression, .. },
                ) if !local.is_mutable && *expression == local.initial_value => {
                    FlowCallResult::ImmutableLocal
                }
                _ => FlowCallResult::Operand,
            };
            checked_call.authored_closing_invocation =
                matches!(call_site, CallSite::TransitionNamed { .. })
                    && closing_invocation(program, state, checked_call.statement_index);
            checked_call.authored_dispatch_role =
                dispatch_calls.role(checked_call.authored_expression);
            let selected = selected_by_statement
                .entry(checked_call.statement_index)
                .or_insert_with(|| {
                    statements
                        .get(checked_call.statement_index)
                        .map(|statement| selected_evaluation(program, machine, statement))
                        .unwrap_or_default()
                });
            checked_call.authored_selected = match &call_site {
                CallSite::Statement(_) => false,
                CallSite::Expression { expression, .. } => {
                    selected.expressions.contains(expression)
                }
                CallSite::TransitionNamed { .. } => selected.transition_targets,
            };
            checked_call.authored_site = site;
            checked_call.authored_target = target;
            checked_call.static_requirement_dispatch =
                call_site.static_requirement_dispatch().map(Into::into);
            checked_call.authored_arguments = control
                .call_arguments
                .insert_many(arguments.iter().copied());
            match authored_call_span(program, &call_site) {
                Ok(source_span) => {
                    checked_call.authored_source_span = source_span;
                    checked_call.authored_source_custody_valid = true;
                }
                Err(_) => {
                    checked_call.authored_source_span = None;
                    checked_call.authored_source_custody_valid = false;
                }
            }
        }
    }
    Ok(())
}

fn authored_call_span(
    program: &dyn ::validation::CheckingInput,
    site: &CallSite<'_>,
) -> Result<Option<SourceSpan>, Vec<Diagnostic>> {
    match site {
        CallSite::Statement(call) => authored_attached_call_span(
            program,
            call.authored_call_selection,
            call.source_span,
            call.operational_acknowledgement.origin,
            "statement",
        ),
        CallSite::Expression { expression, call } => {
            let mut call_occurrences = Vec::new();
            for occurrence in program
                .expression_table
                .authored_selection_occurrences(*expression)
            {
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "expression call retains unknown authored selection occurrence {}",
                        occurrence.ordinal(),
                    ))]);
                };
                if selection.kind() == Kind::Call {
                    call_occurrences.push(occurrence);
                }
            }
            match call.operational_acknowledgement.origin {
                language_semantics::CallOperationalAcknowledgementOrigin::Source => {
                    let [occurrence] = call_occurrences.as_slice() else {
                        return Err(vec![Diagnostic::error(format!(
                            "source-authored checked expression call has {} attached call selections; expected one",
                            call_occurrences.len(),
                        ))]);
                    };
                    validate_occurrence(program, *occurrence, None, "expression").map(Some)
                }
                language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
                    if !call_occurrences.is_empty() {
                        return Err(vec![Diagnostic::error(
                            "compiler-synthesized checked expression call retains authored call provenance",
                        )]);
                    }
                    Ok(None)
                }
            }
        }
        CallSite::TransitionNamed {
            source_span,
            authored_call_selection,
            ..
        } => match (*authored_call_selection, nonempty_span(*source_span)) {
            (Some(occurrence), Some(source_span)) => {
                validate_occurrence(program, occurrence, Some(source_span), "transition").map(Some)
            }
            (None, None) => Ok(None),
            _ => Err(vec![Diagnostic::error(
                "named transition has contradictory authored call provenance",
            )]),
        },
    }
}

fn authored_attached_call_span(
    program: &dyn ::validation::CheckingInput,
    occurrence: Option<OccurrenceId>,
    source_span: SourceSpan,
    origin: language_semantics::CallOperationalAcknowledgementOrigin,
    form: &str,
) -> Result<Option<SourceSpan>, Vec<Diagnostic>> {
    match origin {
        language_semantics::CallOperationalAcknowledgementOrigin::Source => {
            let Some(occurrence) = occurrence else {
                return Err(vec![Diagnostic::error(format!(
                    "source-authored checked {form} call has no attached call selection",
                ))]);
            };
            let Some(source_span) = nonempty_span(source_span) else {
                return Err(vec![Diagnostic::error(format!(
                    "source-authored checked {form} call has no exact source span",
                ))]);
            };
            validate_occurrence(program, occurrence, Some(source_span), form).map(Some)
        }
        language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
            if occurrence.is_some() || nonempty_span(source_span).is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "compiler-synthesized checked {form} call retains authored provenance",
                ))]);
            }
            Ok(None)
        }
    }
}

fn validate_occurrence(
    program: &dyn ::validation::CheckingInput,
    occurrence: OccurrenceId,
    expected_span: Option<SourceSpan>,
    form: &str,
) -> Result<SourceSpan, Vec<Diagnostic>> {
    let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
        return Err(vec![Diagnostic::error(format!(
            "checked {form} call retains unknown authored selection occurrence {}",
            occurrence.ordinal(),
        ))]);
    };
    if selection.kind() != Kind::Call
        || selection.exposure() != Exposure::PrivateImplementation
        || nonempty_span(selection.source_span()).is_none()
        || expected_span.is_some_and(|expected| expected != selection.source_span())
    {
        return Err(vec![
            Diagnostic::error(format!(
                "checked {form} call retains invalid authored call-selection evidence",
            ))
            .with_source_span(selection.source_span()),
        ]);
    }
    Ok(selection.source_span())
}

fn nonempty_span(source_span: SourceSpan) -> Option<SourceSpan> {
    (source_span.span.start < source_span.span.end).then_some(source_span)
}

/// Whether `statement_index` is the state's only transition: its final
/// statement, unconditional, without continuation, exiting ordinarily to a
/// selected named target. A paired unconditional arm after guarded ones is a
/// guarded tail's fallthrough instead.
fn closing_invocation(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> bool {
    use typed_trees::statement::{TransitionExit, TransitionGuardNode, TransitionTargetNode};
    let statements = program.statement_table.statements(state.statement_nodes);
    let Some(StatementNode::Transition(transition)) = statements.get(statement_index) else {
        return false;
    };
    statement_index + 1 == statements.len()
        && !statements[..statement_index]
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        && transition.guard == TransitionGuardNode::Always
        && !transition.continuation.is_valid()
        && transition.exit == TransitionExit::Ordinary
        && program
            .statement_table
            .transition_target_is_valid(transition.target)
        && matches!(
            program.statement_table.transition_target(transition.target),
            TransitionTargetNode::Named { static_machine_parameter, .. }
                if !static_machine_parameter.is_valid()
        )
}

/// One statement's selected evaluation: the expressions that run only on a
/// selected path, and whether its transition targets are selected.
#[derive(Default)]
struct SelectedEvaluation {
    expressions: Vec<typed_trees::expression::ExpressionHandle>,
    transition_targets: bool,
}

/// Mark what runs only on a selected path of `statement`: the right operand
/// of `&&`/`||`, a match arm's pattern and value, and a transition's target
/// and continuation, except an unguarded transition into another machine,
/// whose call every path performs. Selection is inherited by every nested
/// expression.
fn selected_evaluation(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    statement: &StatementNode,
) -> SelectedEvaluation {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    use typed_trees::statement::{TransitionGuardNode, TransitionTargetNode};

    let table = &program.statement_table;
    let mut pending = Vec::new();
    let mut transition_targets = false;
    match statement {
        StatementNode::Call(call) => pending.extend(
            table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| (*argument, false)),
        ),
        StatementNode::LocalData(local) => pending.push((local.initial_value, false)),
        StatementNode::Assignment(assignment) => {
            pending.push((assignment.target, false));
            pending.push((assignment.value, false));
        }
        StatementNode::Expression(expression) => pending.push((*expression, false)),
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                pending.push((guard, false));
            }
            // The machine head and its states are internal jumps, not calls.
            let internal = |target: symbols::SymbolHandle| {
                target == machine.symbol
                    || typed_trees::state::state_by_symbol(program.machine_states(machine), target)
                        .is_some()
            };
            transition_targets = !(transition.guard == TransitionGuardNode::Always
                && matches!(
                    table.transition_target(transition.target),
                    TransitionTargetNode::Named { path, .. } if !internal(path.symbol)
                ));
            for target in transition.targets() {
                match table.transition_target(target) {
                    TransitionTargetNode::Value(expression) => {
                        pending.push((*expression, transition_targets));
                    }
                    TransitionTargetNode::Named { arguments, .. } => pending.extend(
                        table
                            .expression_handles(*arguments)
                            .iter()
                            .map(|argument| (*argument, transition_targets)),
                    ),
                    TransitionTargetNode::Terminal => {}
                }
            }
        }
        _ => {}
    }
    let mut expressions = Vec::new();
    let mut visited = Vec::new();
    while let Some((expression, selected)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            continue;
        }
        visited.push(expression);
        if selected {
            expressions.push(expression);
        }
        let node = program.expression_table.expression(expression);
        let mut position = 0;
        ::validation::expression_traversal::children(program, node, |child| {
            let child_selected = selected
                || match node {
                    ExpressionNode::Binary(binary) => {
                        matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                            && position == 1
                    }
                    // The subject runs on every path; patterns and arm values
                    // run only on the arms they select.
                    ExpressionNode::Match(_) => position > 0,
                    _ => false,
                };
            pending.push((child, child_selected));
            position += 1;
        });
    }
    SelectedEvaluation {
        expressions,
        transition_targets,
    }
}
