//! An aggregate value formed in one evaluation holds every reference leaf at
//! once, just as a call holds every operand, so its leaves face the same
//! simultaneous-hold judgment (`simultaneous`). A `let` or a store forms its
//! owner's leaves at one statement and records them as that statement's
//! loans of that owner. A returning state forms its result at the return;
//! the borrow builder records those leaves separately from local activations,
//! against the current descriptor roster at the actual return statement.
//!
//! Two loans are leaves of one formed value when they share a statement and
//! an owner, sit at different positions of the owner, and come from different
//! source expressions. Loans at one position are alternatives rather than
//! simultaneous holds: a `match` lends each arm's target at the whole value,
//! and a source local holding several loans rebases a leaf onto each
//! candidate. Loans sharing a source expression come from one call result or
//! moved aggregate, whose own formation was judged where it happened. A store
//! through a reference with several candidate destinations repeats each leaf
//! per destination with unchanged places, so each pair of source expressions
//! is reported once.
//!
//! A saved result can also keep the helper's original inputs unavailable.
//! Those restrictions are checked against other references in the aggregate;
//! they are not additional pointers. Views from the same checked call may
//! use their separate referents while outside access keeps those inputs held.

use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    BorrowAccessKind, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts, FlowStateFact,
    StateBorrowFact,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::published::labels::symbol_name;

use super::overlap::StatedOrderingPremise;
use super::simultaneous::{Hold, simultaneous_hold_conflict};

/// One reference leaf of a formed value, borrowed from its loan record.
struct FormedLeaf<'a> {
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
    lineage: &'a checked_trees::BorrowLoanLineage,
    retained_operand: Option<checked_trees::BorrowRetainedOperand>,
    incoming_reference: arena::Handle<checked_trees::IncomingReferenceIdentity>,
    incoming_owner_path: &'a [facts::PlaceSegment],
    owner_symbol: SymbolHandle,
    formation: ExpressionHandle,
    position: &'a [BorrowLoanOwnerSegment],
    root: facts::PlaceRoot,
    segments: &'a [facts::PlaceSegment],
    kind: &'a BorrowAccessKind,
}

impl FormedLeaf<'_> {
    fn formed_with(&self, other: &Self) -> bool {
        self.owner_symbol == other.owner_symbol
            && self.formation != other.formation
            && self.position != other.position
    }

    fn place(&self) -> CapturedPlace {
        CapturedPlace {
            incoming_reference: self.incoming_reference,
            incoming_owner_path: self.incoming_owner_path.to_vec(),
            root: self.root,
            segments: self.segments.to_vec(),
        }
    }

    fn label(&self, program: &dyn ::validation::CheckingInput) -> String {
        facts::canonical_place_label_from_parts(program, self.root, self.segments)
    }
}

pub(super) fn check_formed_aggregates<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_state: &StateBorrowFact,
    stated_premises: &[StatedOrderingPremise],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    // The fact builder appends loans statement by statement, so one
    // statement's loans are contiguous in the state's span.
    let mut offset = 0u32;
    for formed in facts
        .borrow
        .loans
        .span_or_empty(borrow_state.loans)
        .chunk_by(|left, right| left.statement_index == right.statement_index)
    {
        let start = offset;
        offset += formed.len() as u32;
        if formed.len() < 2 {
            continue;
        }
        let leaves: Vec<FormedLeaf<'_>> = formed
            .iter()
            .enumerate()
            .map(|(index, loan)| FormedLeaf {
                loan: borrow_state.loans.handle_at(start + index as u32),
                lineage: &loan.lineage,
                retained_operand: loan.retained_operand,
                incoming_reference: loan.incoming_reference,
                incoming_owner_path: facts
                    .borrow
                    .access_segments
                    .span_or_empty(loan.incoming_owner_path),
                owner_symbol: loan.owner_symbol,
                formation: loan.formation_expression,
                position: facts.borrow.loan_owner_path(loan),
                root: loan.root,
                segments: facts.borrow.loan_segments(loan),
                kind: &loan.kind,
            })
            .collect();
        check_formed_leaves(
            program,
            facts,
            state_flow,
            formed[0].statement_index,
            &leaves,
            |owner| format!("`{}` holds", symbol_name(program, owner)),
            stated_premises,
            call_frames,
            diagnostics,
            bound_lookup,
        );
    }

    let Some((machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };
    for loans in facts
        .borrow
        .returned_loans
        .span_or_empty(borrow_state.returned_loans)
        .chunk_by(|left, right| {
            left.statement_index == right.statement_index
                && left.transition_target == right.transition_target
        })
    {
        if loans.len() < 2 {
            continue;
        }
        let leaves: Vec<FormedLeaf<'_>> = loans
            .iter()
            .map(|loan| FormedLeaf {
                loan: arena::Handle::invalid(),
                lineage: &loan.lineage,
                retained_operand: loan.retained_operand,
                incoming_reference: loan.incoming_reference,
                incoming_owner_path: facts
                    .borrow
                    .access_segments
                    .span_or_empty(loan.incoming_owner_path),
                owner_symbol: state.symbol,
                formation: loan.formation_expression,
                position: facts.borrow.owner_segments.span_or_empty(loan.owner_path),
                root: loan.root,
                segments: facts.borrow.access_segments.span_or_empty(loan.segments),
                kind: &loan.kind,
            })
            .collect();
        let subject = super::escape::state_subject(machine, state);
        check_formed_leaves(
            program,
            facts,
            state_flow,
            loans[0].statement_index,
            &leaves,
            |_| format!("machine `{subject}` returns"),
            stated_premises,
            call_frames,
            diagnostics,
            bound_lookup,
        );
    }
}

/// Judge every pair of leaves formed together at one statement. `subject`
/// renders the diagnostic's subject and verb for the leaves' owner.
fn check_formed_leaves<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement_index: usize,
    leaves: &[FormedLeaf<'_>],
    subject: impl Fn(SymbolHandle) -> String,
    stated_premises: &[StatedOrderingPremise],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let mut premises: Option<Vec<StatedOrderingPremise>> = None;
    let mut reported: Vec<(ExpressionHandle, ExpressionHandle)> = Vec::new();
    for (index, first) in leaves.iter().enumerate() {
        for second in &leaves[index + 1..] {
            if !first.formed_with(second)
                || reported.contains(&(first.formation, second.formation))
                || reported.contains(&(second.formation, first.formation))
            {
                continue;
            }
            // Statement-local call guarantees join the entry premises at the
            // formation point, as they do for that statement's calls.
            let premises = premises.get_or_insert_with(|| {
                let mut available = stated_premises.to_vec();
                super::overlap::append_statement_premises(
                    program,
                    facts,
                    state_flow,
                    statement_index,
                    call_frames,
                    &mut available,
                    bound_lookup,
                );
                available
            });
            let first_debt = lifetime_windows(&facts.borrow, first, leaves);
            let second_debt = lifetime_windows(&facts.borrow, second, leaves);
            if let Some(Err(reason)) = first_debt
                .as_ref()
                .filter(|windows| windows.is_err())
                .or_else(|| second_debt.as_ref().filter(|windows| windows.is_err()))
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{} a reference whose retained input windows are incomplete: {reason}",
                    subject(first.owner_symbol)
                )));
                continue;
            }
            if first_debt.is_some() && second_debt.is_some() {
                // Keeping two input obligations does not borrow either input
                // again. Their actual argument accesses were checked at capture.
                continue;
            }
            if let Some((debt, held, windows)) = first_debt
                .as_ref()
                .map(|windows| (first, second, windows))
                .or_else(|| second_debt.as_ref().map(|windows| (second, first, windows)))
            {
                let windows = match windows {
                    Ok(windows) => windows,
                    Err(reason) => {
                        diagnostics.push(Diagnostic::error(format!(
                            "{} a reference whose retained input windows are incomplete: {reason}",
                            subject(first.owner_symbol)
                        )));
                        continue;
                    }
                };
                // Sibling views can use their own completed result. This
                // correspondence does not authorize access through its lender,
                // and the physical leaves still face their ordinary pair checks.
                if shared_result_access(program, facts, debt, held, leaves, premises, bound_lookup)
                {
                    continue;
                }
                for window in windows {
                    if let Some(conflict) = simultaneous_hold_conflict(
                        program,
                        &facts.borrow,
                        &window.place,
                        &window.access,
                        &held.place(),
                        held.kind,
                        premises,
                        bound_lookup,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "{} {}",
                            subject(first.owner_symbol),
                            conflict.phrase(
                                |hold| match hold {
                                    Hold::First => facts::canonical_place_label_from_parts(
                                        program,
                                        window.place.root,
                                        &window.place.segments
                                    ),
                                    Hold::Second => held.label(program),
                                },
                                "another of its references"
                            )
                        )));
                        reported.push((first.formation, second.formation));
                        break;
                    }
                }
                continue;
            }
            let Some(conflict) = simultaneous_hold_conflict(
                program,
                &facts.borrow,
                &first.place(),
                first.kind,
                &second.place(),
                second.kind,
                premises,
                bound_lookup,
            ) else {
                continue;
            };
            reported.push((first.formation, second.formation));
            let label = |hold| match hold {
                Hold::First => first.label(program),
                Hold::Second => second.label(program),
            };
            diagnostics.push(Diagnostic::error(format!(
                "{} {}",
                subject(first.owner_symbol),
                conflict.phrase(label, "another of its references"),
            )));
        }
    }
}

fn lifetime_windows(
    borrow: &checked_trees::BorrowFacts,
    debt: &FormedLeaf<'_>,
    leaves: &[FormedLeaf<'_>],
) -> Option<Result<Vec<super::resources::supplied_permissions::HeldInput>, &'static str>> {
    if *debt.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency {
        return None;
    }
    if debt.loan.is_valid()
        && let Some(windows) =
            super::resources::supplied_permissions::debt_windows(borrow, debt.loan)
        {
            return Some(windows);
        }
    let mut windows = Vec::new();
    let mut completed = false;
    let mut other = false;
    for peer in leaves.iter().filter(|peer| {
        *peer.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
            && peer.owner_symbol == debt.owner_symbol
            && peer.position == debt.position
            && peer.formation == debt.formation
    }) {
        if let Some(found) =
            super::resources::supplied_permissions::output_windows(borrow, peer.incoming_reference)
        {
            completed = true;
            match found {
                Ok(found) => windows.extend(found),
                Err(reason) => return Some(Err(reason)),
            }
        } else {
            other = true;
        }
    }
    if completed {
        return Some(if other {
            Err("the returned value has unjoined input generations")
        } else {
            Ok(windows)
        });
    }
    // Older entry/returned-input records already carry their input region.
    // Treat it as a retention demand, never as another physical reference leaf.
    Some(Ok(vec![
        super::resources::supplied_permissions::HeldInput {
            place: debt.place(),
            access: debt.kind.clone(),
        },
    ]))
}

fn shared_result_access<'p>(
    program: &'p dyn validation::CheckingInput,
    facts: &CheckFacts,
    debt: &FormedLeaf<'_>,
    held: &FormedLeaf<'_>,
    leaves: &[FormedLeaf<'_>],
    premises: &[StatedOrderingPremise],
    lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    let Some(retained) = debt
        .retained_operand
        .filter(|retained| retained.formation_expression.is_valid())
    else {
        return false;
    };
    if held.retained_operand != Some(retained) {
        return false;
    }
    let peers = leaves
        .iter()
        .filter(|peer| {
            *peer.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                && peer.owner_symbol == debt.owner_symbol
                && peer.position == debt.position
                && peer.formation == debt.formation
        })
        .collect::<Vec<_>>();
    !peers.is_empty()
        && peers.iter().all(|peer| {
            peer.retained_operand == Some(retained)
                && super::overlap::captured_place_compatibility(
                    program,
                    &facts.borrow,
                    &peer.place(),
                    peer.kind,
                    &held.place(),
                    held.kind,
                    premises,
                    lookup,
                )
                .non_interfering
        })
}
