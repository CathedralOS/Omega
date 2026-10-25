//! Frozen reference leaves held by completed actuals. Unknown address coverage
//! remains unknown; lifetime dependencies never substitute for referents.
use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep};
use arena::Handle;
use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};

pub(super) struct HeldLeaf {
    pub(super) expression: typed_trees::expression::ExpressionHandle,
    pub(super) completion: usize,
    pub(super) route: Option<usize>,
    pub(super) until: usize,
    pub(super) copied: bool,
    pub(super) access: BorrowAccessKind,
    pub(super) place: Option<CapturedPlace>,
}

pub(super) fn append(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: Handle<FlowOperandFact>,
    until: usize,
    copied_parent: bool,
    occurrences: &[StatementOccurrence],
    leaves: &mut Vec<HeldLeaf>,
) {
    let capture = facts.flow.control.operands.get(operand);
    let copied = copied_parent
        || crate::borrow::accesses::is_owned_copy_type(program, capture.type_reference);
    if !capture.alternatives.is_empty() {
        for alternative in capture.alternatives.handles() {
            append(
                program,
                facts,
                state,
                alternative,
                until,
                copied,
                occurrences,
                leaves,
            );
        }
        return;
    }
    if !capture.children.is_empty() {
        for child in facts
            .flow
            .control
            .operand_children
            .span_or_empty(capture.children)
        {
            append(
                program,
                facts,
                state,
                child.value,
                until,
                copied,
                occurrences,
                leaves,
            );
        }
        return;
    }
    // A saved dispatch operand can precede this statement's roster. Its
    // demand is already live; missing a new completion never releases it.
    let mut completions = occurrences.iter().enumerate().filter(|(position, occurrence)| {
        *position < until && matches!(occurrence.step, StatementStep::Complete(expression) if expression == capture.expression)
    });
    let first = completions.next().map(|(position, _)| position);
    let route = if completions.next().is_some() {
        None
    } else {
        first
    };
    let completion = route.unwrap_or(0);
    let Some(paths) =
        validation::borrow_contracts::carried_reference_paths(program, capture.type_reference)
    else {
        leaves.push(HeldLeaf {
            expression: capture.expression,
            completion,
            route,
            until,
            copied,
            access: BorrowAccessKind::Mutable,
            place: None,
        });
        return;
    };
    for (path, access) in paths {
        let path = crate::borrow::owner_place_segments(&path);
        // The declared frontier includes every case, but a completed sum
        // retains only its active payload. Consume contrary tag evidence at
        // this capture's own epoch; missing address evidence alone proves no
        // absence (including image/provider-backed descriptors).
        if path
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
            && let Some(mut source) = validation::canonical_place_from_expression_in_state(
                program,
                capture.state_symbol,
                capture.statement_index,
                capture.expression,
            )
            && let Some(machine) = program.machine_holding_state(capture.state_symbol)
        {
            source.segments.extend_from_slice(&path);
            let contexts = facts
                .flow
                .semantic_constraint_contexts(capture.constraints)
                .collect::<Vec<_>>();
            if crate::flow::place_case_path_is_inactive(
                program,
                &facts.semantic,
                &contexts,
                machine.symbol,
                capture.state_symbol,
                capture.statement_index,
                &source,
            ) {
                continue;
            }
        }
        let places =
            super::super::captured_descriptors::places(program, facts, state, operand, &path);
        match places {
            Some(places) => {
                for place in places {
                    leaves.push(HeldLeaf {
                        expression: capture.expression,
                        completion,
                        route,
                        until,
                        copied,
                        access: access.clone(),
                        place: Some(place),
                    });
                }
            }
            None => leaves.push(HeldLeaf {
                expression: capture.expression,
                completion,
                route,
                until,
                copied,
                access,
                place: None,
            }),
        }
    }
}
