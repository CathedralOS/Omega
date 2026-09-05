use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
    AuthoredDeclarationSelectionOccurrenceId as OccurrenceId,
};
use source::SourceSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

use crate::lookup::{
    call_receiver_parts, resolve_state_call_target, statement_call_receiver_members,
};
use crate::semantic_calls::{CallSite, find_call_site};

pub(super) fn bind_checked_body_call_source_spans(
    program: &TypedTrees,
    flow: &mut checked_trees::FlowFacts,
) -> Result<(), Vec<Diagnostic>> {
    let checked_states = flow
        .control
        .states
        .iter()
        .map(|(_, state)| state.clone())
        .collect::<Vec<_>>();
    for checked_state in checked_states {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == checked_state.machine_symbol)
        else {
            return Err(vec![Diagnostic::error(format!(
                "checked machine {:?} is absent while binding call source custody",
                checked_state.machine_symbol,
            ))]);
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == checked_state.state_symbol)
        else {
            return Err(vec![Diagnostic::error(format!(
                "checked state {:?} is absent from machine {:?} while binding call source custody",
                checked_state.state_symbol, checked_state.machine_symbol,
            ))]);
        };
        for checked_call in flow.control.calls.span_mut_or_empty(checked_state.calls) {
            let Some(call_site) = find_call_site(
                program,
                checked_state.machine_symbol,
                checked_state.state_symbol,
                checked_call.statement_index,
                checked_call.call_ordinal,
            ) else {
                return Err(vec![Diagnostic::error(format!(
                    "checked body call {} in statement {} of machine {:?} state {:?} has no exact typed call site",
                    checked_call.call_ordinal,
                    checked_call.statement_index,
                    checked_state.machine_symbol,
                    checked_state.state_symbol,
                ))]);
            };
            validate_checked_call_join(program, machine, state, checked_call, &call_site)?;
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

pub(super) fn derive_checked_body_call_source_spans(
    _program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine_symbol: SymbolHandle,
) -> Result<Vec<SourceSpan>, Vec<Diagnostic>> {
    let mut spans = Vec::new();
    for (_, checked_state) in facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine_symbol)
    {
        for checked_call in facts.flow.control.calls.span_or_empty(checked_state.calls) {
            if !checked_call.authored_source_custody_valid {
                return Err(vec![Diagnostic::error(format!(
                    "checked body call {} in statement {} has invalid retained source custody",
                    checked_call.call_ordinal, checked_call.statement_index,
                ))]);
            }
            match (
                checked_call.operational_acknowledgement.origin,
                checked_call.authored_source_span.and_then(nonempty_span),
            ) {
                (
                    language_semantics::CallOperationalAcknowledgementOrigin::Source,
                    Some(source_span),
                ) => spans.push(source_span),
                (
                    language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                    None,
                ) => {}
                _ => {
                    return Err(vec![Diagnostic::error(format!(
                        "checked body call {} in statement {} has contradictory retained source custody",
                        checked_call.call_ordinal, checked_call.statement_index,
                    ))]);
                }
            }
        }
    }
    Ok(spans)
}

fn validate_checked_call_join(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    checked: &checked_trees::FlowCallFact,
    site: &CallSite<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let (receiver_symbol, target_symbol, has_receiver, acknowledgement) = match site {
        CallSite::Statement(call) => {
            let resolved_target = resolve_state_call_target(
                program,
                machine,
                state,
                call.receiver_symbol,
                call.target_symbol,
                statement_call_receiver_members(program, call),
                &call.target,
            );
            (
                call.receiver_symbol,
                if resolved_target.is_valid() {
                    resolved_target
                } else {
                    call.target_symbol
                },
                !call.receiver.is_empty(),
                call.operational_acknowledgement,
            )
        }
        CallSite::Expression { call, .. } => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(program, call.receiver);
            let resolved_target = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            );
            (
                receiver_symbol,
                if resolved_target.is_valid() {
                    resolved_target
                } else {
                    call.target_symbol
                },
                receiver_path.is_some(),
                call.operational_acknowledgement,
            )
        }
        CallSite::TransitionNamed { path, .. } => (
            path.head_symbol,
            path.symbol,
            path.members.count() > 1,
            Default::default(),
        ),
    };
    if checked.receiver_symbol != receiver_symbol
        || checked.target_symbol != target_symbol
        || checked.has_receiver != has_receiver
        || checked.operational_acknowledgement != acknowledgement
    {
        return Err(vec![Diagnostic::error(format!(
            "checked body-call identity does not match typed call site at statement {} ordinal {}",
            checked.statement_index, checked.call_ordinal,
        ))]);
    }
    Ok(())
}

fn authored_call_span(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
    program: &TypedTrees,
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
