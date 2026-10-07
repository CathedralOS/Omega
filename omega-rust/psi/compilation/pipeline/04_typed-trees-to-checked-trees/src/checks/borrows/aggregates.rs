//! An aggregate value formed in one evaluation holds every reference leaf at
//! once, just as a call holds every operand, so its leaves face the same
//! simultaneous-hold judgment (`simultaneous`). A `let` or a store forms its
//! owner's leaves at one statement and records them as that statement's
//! loans of that owner. A returning state forms its result at the return;
//! `borrow::returned_value_loans` resolves those leaves with the same loan
//! builder, since a returned value has no local owner to record them under.
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
    owner_symbol: SymbolHandle,
    formation: ExpressionHandle,
    position: &'a [BorrowLoanOwnerSegment],
    root_symbol: SymbolHandle,
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
            root_symbol: self.root_symbol,
            segments: self.segments.to_vec(),
        }
    }

    fn label(&self, program: &typed_trees::TypedTrees) -> String {
        facts::canonical_place_label_from_parts(
            program,
            facts::PlaceRoot::Symbol(self.root_symbol),
            self.segments,
        )
    }
}

pub(super) fn check_formed_aggregates<'p>(
    program: &'p typed_trees::TypedTrees,
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
                owner_symbol: loan.owner_symbol,
                formation: loan.formation_expression,
                position: facts.borrow.loan_owner_path(loan),
                root_symbol: loan.root_symbol,
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

    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return;
    };
    let Some(state) = crate::semantic::calls::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let Some(expression) = super::escape::return_expression(program, statement) else {
            continue;
        };
        let loans = crate::borrow::returned_value_loans(
            program,
            machine.symbol,
            state,
            &facts.borrow,
            borrow_state,
            statement_index,
            expression,
        );
        if loans.len() < 2 {
            continue;
        }
        let leaves: Vec<FormedLeaf<'_>> = loans
            .iter()
            .map(|loan| FormedLeaf {
                owner_symbol: state.symbol,
                formation: loan.formation_expression,
                position: &loan.owner_path,
                root_symbol: loan.place.root_symbol,
                segments: &loan.place.segments,
                kind: &loan.kind,
            })
            .collect();
        let subject = super::escape::state_subject(machine, state);
        check_formed_leaves(
            program,
            facts,
            state_flow,
            statement_index,
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
    program: &'p typed_trees::TypedTrees,
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
