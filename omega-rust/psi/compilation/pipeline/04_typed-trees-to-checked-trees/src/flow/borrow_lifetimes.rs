//! Borrow-loan expiry in the active constraints. `filter_expired_borrow_loans`
//! drops loans past their last use, and `filter_reassigned_borrow_loans` drops
//! loans carried by the place an assignment overwrites. Each dropped loan
//! records a `FlowBorrowWeakeningFact`.

use validation::CanonicalPlace;

use crate::flow::reference_spans;
use checked_trees::{
    BorrowFacts, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowConstraintKind,
    FlowConstraintRef, FlowInvalidationSource,
};

pub(crate) fn filter_expired_borrow_loans(
    borrow_weakenings: &mut arena::PagedArenaSet<FlowBorrowWeakeningFact>,
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    statement_index: usize,
    execution_order: &typed_trees::statement::StatementExecutionOrder,
    reason: FlowBorrowWeakeningReason,
) -> arena::HandleSpan<FlowConstraintRef> {
    if !constraint_refs
        .span_or_empty(source)
        .iter()
        .any(|constraint| matches!(constraint.kind, FlowConstraintKind::BorrowLoan { .. }))
    {
        return source;
    }
    reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let loan_fact = borrow.loans.get(loan);
                let current = execution_order
                    .position(statement_index)
                    .expect("loan expiry names an admitted execution point or state exit");
                let keep = execution_order
                    .position(loan_fact.last_use_statement_index)
                    .is_none_or(|last_use| last_use >= current);
                if !keep {
                    // Record the exact successor source point, which may be
                    // earlier in the source file after an assembly edge.
                    // Independent replay uses the same admitted control order.
                    borrow_weakenings.append(FlowBorrowWeakeningFact {
                        source: FlowInvalidationSource::Statement {
                            statement_index: execution_order
                                .successor(loan_fact.last_use_statement_index)
                                .expect("an expired loan has a following execution boundary"),
                        },
                        loan,
                        reason,
                    });
                }
                keep
            }
            FlowConstraintKind::Unknown | checked_trees::nonloan_constraint_pattern!(..) => true,
        }
    })
}

/// The destination is already resolved by completed operand capture and must
/// name one exact, still-current slot. Both flow construction and independent
/// replay prepare their resolver before
/// traversing statements. `None` is that immutable preparation's failed result,
/// not a request to reconstruct the resolver for each loan-lifetime query.
pub(crate) fn filter_reassigned_borrow_loans(
    borrow_weakenings: &mut arena::PagedArenaSet<FlowBorrowWeakeningFact>,
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    borrow: &BorrowFacts,
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    captured_destination: Option<CanonicalPlace>,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> arena::HandleSpan<FlowConstraintRef> {
    // Calls on the lexical continuation publish one current occurrence per
    // exact slot. Outgoing-arm replacements do not retire the guard's retained
    // fallthrough inventory; their selected call already owns that custody.
    // Completed calls publish one current occurrence for each exact slot,
    // including incoming alternatives on no-write routes. Their former
    // occurrences retire only after RHS formation. Weak stores leave other
    // physical slots live. Then apply any enclosing assignment's store.
    if !constraint_refs
        .span_or_empty(source)
        .iter()
        .any(|constraint| {
            matches!(
                constraint.kind,
                FlowConstraintKind::BorrowLoan { .. }
                    | FlowConstraintKind::BorrowIncomingReference { .. }
            )
        })
    {
        return source;
    }
    let source = if let Some(effects) = call_frames
        .and_then(|frames| frames.stored_reference_fallthrough_effects(machine, statement))
    {
        reference_spans::filter_constraint_refs(constraint_refs, source, |constraint| {
            if let FlowConstraintKind::BorrowIncomingReference { identity } = constraint.kind {
                let incoming = borrow.incoming_references.get(identity);
                return !effects.iter().any(|effect| {
                    !effect.may_select
                        && facts::PlaceRoot::Symbol(incoming.parameter_symbol())
                            == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                        && crate::borrow::place_path_covers_owner(
                            program,
                            &effect.destination.local_segments,
                            borrow.owner_segments.span_or_empty(incoming.owner_path),
                        )
                });
            }
            let FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
                return true;
            };
            let active = borrow.loans.get(loan);
            let replaced = active.statement_index < statement_index
                && effects.iter().any(|effect| {
                    !effect.may_select
                        && active.storage_owner
                            == facts::PlaceRoot::Symbol(effect.destination.local_symbol)
                        && crate::borrow::place_path_covers_owner(
                            program,
                            &effect.destination.local_segments,
                            borrow.loan_owner_path(active),
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
        })
    } else {
        source
    };
    let Some(destination) = captured_destination else {
        return source;
    };
    // Reference carriers have the same lifetime rule at a root or a projection:
    // writing through one preserves its captured loan, while replacing it ends
    // the old occurrence. Classify the typed target before comparing owner paths.
    let mut writes_referent = false;
    if let Some(assignment) = statement.as_assignment() {
        let Some(mut reference) =
            validation::declared_place_type_raw(program, machine, Some(state), assignment.target)
        else {
            return source;
        };
        while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        if matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        ) {
            let Some(replacement) = call_frames.and_then(|frames| {
                frames.assignment_replaces_reference_binding(machine, statement)
            }) else {
                return source;
            };
            writes_referent = !replacement;
        }
    }

    // The caller supplies the one completed, still-current address. It has
    // already crossed reference ancestors; resolving it again using the live
    // loan set could redirect it to a different slot.
    let authored_carrier = statement.as_assignment().and_then(|assignment| {
        validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            assignment.target,
        )
    });

    if writes_referent && authored_carrier.is_none() {
        return source;
    }
    if !constraint_refs
        .span_or_empty(source)
        .iter()
        .any(|constraint| {
            matches!(
                constraint.kind,
                FlowConstraintKind::BorrowLoan { .. }
                    | FlowConstraintKind::BorrowIncomingReference { .. }
            )
        })
    {
        return source;
    }
    reference_spans::filter_constraint_refs(constraint_refs, source, |constraint_ref| {
        match constraint_ref.kind {
            FlowConstraintKind::BorrowLoan { loan } => {
                let active = borrow.loans.get(loan);
                // Loans established by this assignment describe the replacement
                // value and must survive it. Older loans carried by the
                // overwritten whole/place are invalidated precisely.
                let keep = active.storage_owner != destination.root
                    || (writes_referent
                        && authored_carrier.as_ref() == Some(&destination)
                        && borrow.loan_owner_path(active).len() == destination.segments.len())
                    || active.statement_index == statement_index
                    || !crate::borrow::place_path_covers_owner(
                        program,
                        &destination.segments,
                        borrow.loan_owner_path(active),
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
            FlowConstraintKind::BorrowIncomingReference { identity } => {
                let incoming = borrow.incoming_references.get(identity);
                let owner = borrow.owner_segments.span_or_empty(incoming.owner_path);
                // Canonical roots retain opaque invocation referent/output
                // namespaces; only this parameter's descriptor slot can die.
                destination.root != facts::PlaceRoot::Symbol(incoming.parameter_symbol())
                    || (writes_referent
                        && authored_carrier.as_ref() == Some(&destination)
                        && owner.len() == destination.segments.len())
                    || !crate::borrow::place_path_covers_owner(
                        program,
                        &destination.segments,
                        owner,
                    )
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
