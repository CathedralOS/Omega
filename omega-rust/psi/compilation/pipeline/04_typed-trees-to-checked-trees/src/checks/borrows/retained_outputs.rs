//! Attribution of completed receiving storage, separate from whether its
//! loans are still live. A temporary argument's borrow ends at invocation;
//! only an actual retained output owes a continuing loan. Missing captures or
//! lifetime frontiers cannot certify absence. A matching historical root is
//! insufficient: the receiving storage path and establishment statement must
//! identify the produced occurrence whose lifetime the flow ledger tracks.

use checked_trees::{CheckFacts, FlowStateFact};
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use validation::CanonicalPlace;

pub(crate) fn retained_outputs_are_attributed(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    source: SymbolHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(completed) = facts.flow.state_statement(state_flow, statement_index) else {
        return false;
    };
    let control = &facts.flow.control;
    if matches!(
        statement,
        typed_trees::statement::StatementNode::LocalData(_)
            | typed_trees::statement::StatementNode::Assignment(_)
    ) && !control.operands.is_valid(completed.stored_value)
    {
        return false;
    }
    if control.operands.is_valid(completed.stored_value) {
        let value = control.operands.get(completed.stored_value);
        let Some(paths) = validation::borrow_contracts::borrow_carrying_owner_paths(
            program,
            value.type_reference,
        ) else {
            return false;
        };
        for path in paths {
            let path = crate::borrow::owner_place_segments(&path);
            if inactive_path(program, facts, state_flow, value, &path) {
                continue;
            }
            let Some(profiles) = validation::completed_storage::captured_retention_profiles_at_path(
                program, control, value, &path,
            ) else {
                return false;
            };
            let Some(destination) = facts
                .semantic
                .places
                .is_valid(completed.stored_destination)
                .then(|| facts.semantic.places.get(completed.stored_destination))
                .and_then(|place| {
                    crate::flow::canonical_place_from_semantic_place(
                        program,
                        &facts.semantic,
                        place,
                    )
                })
            else {
                return false;
            };
            let Some(destination) =
                validation::completed_storage::project_captured_view_place(destination, &path)
            else {
                return false;
            };
            for profile in profiles {
                let Some(roots) = control.retained_roots(profile) else {
                    return false;
                };
                if roots.contains(&PlaceRoot::Symbol(source))
                    && !attributed(facts, state_flow, statement_index, &destination, source)
                {
                    return false;
                }
            }
        }
    }
    let Some(effects) =
        frames.and_then(|frames| frames.stored_reference_effects(machine, statement))
    else {
        return false;
    };
    for effect in effects {
        let destination = CanonicalPlace {
            root: PlaceRoot::Symbol(effect.destination.local_symbol),
            segments: effect.destination.local_segments,
        };
        // Inspect the produced post-call descriptor, including retained
        // incoming alternatives and lifetime-only dependencies. Reinterpreting
        // the relation's source geometry here would duplicate loan formation
        // and could erase opaque lifetime debt or a no-write alternative.
        let Some(captures) = validation::completed_storage::at_snapshot(
            program,
            control,
            completed.storage_snapshot,
            &destination,
            validation::completed_storage::StoredObservation::Binding,
        )
        .and_then(|stored| stored.captures()) else {
            return false;
        };
        for capture in captures {
            let value = control.operands.get(capture.capture);
            if inactive_path(program, facts, state_flow, value, &capture.projection) {
                continue;
            }
            let Some(profiles) = validation::completed_storage::captured_retention_profiles_at_path(
                program,
                control,
                value,
                &capture.projection,
            ) else {
                return false;
            };
            for profile in profiles {
                let Some(roots) = control.retained_roots(profile) else {
                    return false;
                };
                if roots.contains(&PlaceRoot::Symbol(source))
                    && !attributed(facts, state_flow, statement_index, &destination, source)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn attributed(
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    destination: &CanonicalPlace,
    source: SymbolHandle,
) -> bool {
    let Some((_, borrowing)) =
        crate::flow::borrow_state_fact(&facts.borrow, state.machine_symbol, state.state_symbol)
    else {
        return false;
    };
    facts.borrow.loans.iter().any(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrowing, handle)
            && loan.statement_index == statement_index
            && loan.storage_owner == destination.root
            && loan.root == PlaceRoot::Symbol(source)
            && owner_path_matches(facts, loan, &destination.segments)
    })
}

fn owner_path_matches(
    facts: &CheckFacts,
    loan: &checked_trees::BorrowLoanFact,
    path: &[PlaceSegment],
) -> bool {
    crate::borrow::owner_place_segments(facts.borrow.loan_owner_path(loan)) == path
}

/// A possible type leaf is not necessarily present in this completed value.
/// Consume its captured case fact, without rereading later mutable storage or
/// inferring presence/absence from a reference-shaped type. A reference
/// crossing ends this snapshot: the pointee's tag may have changed since then.
fn inactive_path(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: &checked_trees::FlowOperandFact,
    path: &[PlaceSegment],
) -> bool {
    let Some(mut subject) = validation::canonical_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    ) else {
        return false;
    };
    let contexts = facts
        .flow
        .semantic_constraint_contexts(operand.constraints)
        .collect::<Vec<_>>();
    let mut reference = operand.type_reference;
    for segment in path {
        if validation::reference_shell(program, reference).is_some() {
            return false;
        }
        if let PlaceSegment::Case { variant } = segment
            && crate::flow::place_case_has_value(
                program,
                &facts.semantic,
                &contexts,
                state.machine_symbol,
                operand.state_symbol,
                operand.statement_index,
                &subject,
                *variant,
                false,
            )
        {
            return true;
        }
        let Some(projected) = validation::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        ) else {
            return false;
        };
        reference = projected;
        let Some(projected) = validation::completed_storage::project_captured_view_place(
            subject,
            std::slice::from_ref(segment),
        ) else {
            return false;
        };
        subject = projected;
    }
    false
}
