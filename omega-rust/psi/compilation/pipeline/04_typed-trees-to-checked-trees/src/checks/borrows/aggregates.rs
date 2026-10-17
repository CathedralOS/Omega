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

use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    BorrowAccessKind, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts, FlowStateFact,
    StateBorrowFact,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::labels::symbol_name;

use super::overlap::StatedOrderingPremise;
use super::simultaneous::{Hold, simultaneous_hold_conflict};

/// One reference leaf of a formed value, borrowed from its loan record.
struct FormedLeaf<'a> {
    incoming_reference: arena::Handle<checked_trees::IncomingReferenceIdentity>,
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
    for formed in facts
        .borrow
        .loans
        .span_or_empty(borrow_state.loans)
        .chunk_by(|left, right| left.statement_index == right.statement_index)
    {
        if formed.len() < 2 {
            continue;
        }
        let leaves: Vec<FormedLeaf<'_>> = formed
            .iter()
            .map(|loan| FormedLeaf {
                incoming_reference: loan.incoming_reference,
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
                incoming_reference: loan.incoming_reference,
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
                super::overlap::append_call_premises(
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
            let Some(conflict) = simultaneous_hold_conflict(
                program,
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
