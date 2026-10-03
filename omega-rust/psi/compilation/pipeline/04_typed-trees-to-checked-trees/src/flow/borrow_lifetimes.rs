//! Borrow-loan expiry in the active constraints. `filter_expired_borrow_loans`
//! drops loans past their last use, and `filter_reassigned_borrow_loans` drops
//! loans carried by the place an assignment overwrites. Each dropped loan
//! records a `FlowBorrowWeakeningFact`.
use crate::flow::CanonicalPlace;
use crate::flow::reference_spans;
use checked_trees::{
    BorrowFacts, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowConstraintKind,
    FlowConstraintRef, FlowInvalidationSource,
};

pub(crate) fn filter_expired_borrow_loans(
    borrow_weakenings: &mut arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    statement_index: usize,
    reason: FlowBorrowWeakeningReason,
) -> arena::HandleSpan<FlowConstraintRef> {
    reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let loan_fact = borrow.loans.get(loan);
                let keep = loan_fact.last_use_statement_index >= statement_index;
                if !keep {
                    // The weakening boundary is a recorded coordinate — one
                    // statement past the loan's recorded last use — never the
                    // position this traversal happened to filter at. In the
                    // ordinary per-statement walk the two coincide (the filter
                    // fires at `last_use + 1`, and the state-exit pass reaches
                    // only loans whose boundary is the statement count), but
                    // stamping the recorded boundary keeps the emitted fact a
                    // function of the loan row alone, so a replayed derivation
                    // — `receiver_aliases` re-derives exactly
                    // `last_use_statement_index + 1` — reaches the same
                    // evidence under any filtering cadence.
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement {
                            statement_index: loan_fact.last_use_statement_index + 1,
                        },
                        loan,
                        reason,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown
            | FlowConstraintKind::SemanticContext { .. }
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. } => true,
        }
    })
}

pub(crate) fn filter_reassigned_borrow_loans(
    borrow_weakenings: &mut arena::Arena<FlowBorrowWeakeningFact>,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    program: &typed_trees::TypedTrees,
    reassigned_place: Option<CanonicalPlace>,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> arena::HandleSpan<FlowConstraintRef> {
    if matches!(statement, typed_trees::statement::StatementNode::Call(_)) {
        let machine = program.machines().iter().find(|machine| {
            program.machine_states(machine).iter().any(|state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .get(statement_index)
                    .is_some_and(|candidate| std::ptr::eq(candidate, statement))
            })
        });
        let mut owned_frames = None;
        let effects = machine.and_then(|machine| {
            crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
                .and_then(|frames| frames.stored_reference_effects(machine, statement))
        });
        let Some(effects) = effects else {
            return source;
        };
        return reference_spans::filter_constraint_refs(constraint_refs, source, |constraint| {
            let FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
                return true;
            };
            let active = borrow.loans.get(loan);
            let replaced = active.statement_index < statement_index
                && effects.iter().any(|effect| {
                    !effect.retains_incoming
                        && active.owner_symbol == effect.destination.local_symbol
                        && borrow_owner_path_overlaps_place(
                            program,
                            borrow.loan_owner_path(active),
                            &effect.destination.local_segments,
                        )
                });
            if replaced {
                borrow_weakenings.append(FlowBorrowWeakeningFact {
                    source: FlowInvalidationSource::Statement { statement_index },
                    loan,
                    reason: FlowBorrowWeakeningReason::LocalReassigned,
                });
            }
            !replaced
        });
    }
    let Some(place) = reassigned_place else {
        return source;
    };
    let (reassigned_symbol, reassigned_segments) = match place.root {
        facts::PlaceRoot::Symbol(symbol) => (symbol, place.segments),
        _ => return source,
    };

    // A bare reference local may be assigned through rather than replaced.
    // Retire its carried loan only when the ordinary assignment classification
    // establishes a binding replacement; writing the pointee keeps the loan.
    if reassigned_segments.is_empty()
        && let Some((machine, state)) = crate::semantic::calls::find_state_with_machine(
            program,
            program.symbols.get(reassigned_symbol).parent,
        )
        && let Some((_, local)) = program.state_local_declaration(state, reassigned_symbol)
    {
        let mut reference = local.type_reference;
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        if matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            let mut owned_frames = None;
            let replacement =
                crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
                    .and_then(|frames| {
                        frames.assignment_replaces_local_reference_binding(machine, statement)
                    });
            if replacement != Some(true) {
                return source;
            }
        }
    }

    let target = checked_trees::CapturedPlace {
        root_symbol: reassigned_symbol,
        segments: reassigned_segments,
    };
    let destinations = crate::semantic::calls::find_state_with_machine(
        program,
        program.symbols.get(reassigned_symbol).parent,
    )
    .map(|(_, state)| {
        crate::borrow::store_destination_places(
            program,
            state,
            target.clone(),
            constraint_refs
                .span_or_empty(source)
                .iter()
                .filter_map(|constraint| match constraint.kind {
                    FlowConstraintKind::BorrowLoan { loan } => Some(borrow.loans.get(loan)),
                    _ => None,
                })
                .filter(|loan| loan.statement_index < statement_index)
                .map(|loan| {
                    (
                        loan.owner_symbol,
                        borrow.loan_owner_path(loan),
                        loan.root_symbol,
                        borrow.loan_segments(loan),
                    )
                }),
        )
    })
    .unwrap_or_else(|| vec![target]);
    let [destination] = destinations.as_slice() else {
        // A may-alias set cannot prove which old slot was replaced.
        return source;
    };

    reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let active = borrow.loans.get(loan);
                // Loans established by this assignment describe the replacement
                // value and must survive it. Older loans carried by the
                // overwritten whole/place are invalidated precisely.
                let keep = active.owner_symbol != destination.root_symbol
                    || active.statement_index == statement_index
                    || !borrow_owner_path_overlaps_place(
                        program,
                        borrow.loan_owner_path(active),
                        &destination.segments,
                    );
                if !keep {
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement { statement_index },
                        loan,
                        reason: FlowBorrowWeakeningReason::LocalReassigned,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown
            | FlowConstraintKind::SemanticContext { .. }
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. } => true,
        }
    })
}

fn borrow_owner_path_overlaps_place(
    program: &typed_trees::TypedTrees,
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    place_segments: &[facts::PlaceSegment],
) -> bool {
    // An assignment retires loans carried by the exact target or anything
    // nested inside it. A loan carried by the whole owner (empty path) is not
    // retired by assigning through that owner's pointee/field; only replacing
    // the whole owner does that.
    place_segments.len() <= owner_path.len()
        && owner_path
            .iter()
            .zip(place_segments)
            .all(|(owner, place)| match (owner, place) {
                (
                    checked_trees::BorrowLoanOwnerSegment::Field(owner_symbol),
                    facts::PlaceSegment::Field {
                        symbol: place_symbol,
                    },
                ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
                (
                    checked_trees::BorrowLoanOwnerSegment::Case(owner_variant),
                    facts::PlaceSegment::Case {
                        variant: place_variant,
                    },
                ) => owner_variant == place_variant,
                (
                    checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::FixedIndex { index: place_index },
                ) => owner_index == place_index,
                (
                    checked_trees::BorrowLoanOwnerSegment::FixedIndex(owner_index),
                    facts::PlaceSegment::Index { expression },
                ) => program
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_none_or(|place_index| *owner_index == place_index),
                (
                    checked_trees::BorrowLoanOwnerSegment::DynamicIndex,
                    facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
                ) => true,
                _ => false,
            })
}
