//! Statement reads under live loans.
//!
//! A read observes the storage it names, so an exclusive loan that suspends
//! that storage excludes it until the loan's last use; a shared loan freezes
//! only mutation. Every evaluated observation of a statement, in the order
//! `ownership_occurrences` replays it, is judged by the shared place-access
//! judgment against the loans live where it is evaluated: those live at the
//! statement's entry and those the statement already formed on a compatible
//! route. A borrow expression accesses its place with its own access kind.
//!
//! A loan whose last use is this statement ends inside it, once the last
//! observation naming its owner is no longer held by an invocation; a read
//! evaluated after that point is a read after the last use. When the use is
//! not such an observation (an assignment destination, a projected carrier),
//! the loan stays live through the whole statement.
//!
//! Two observations are judged at their own event instead. The place a loan
//! forms over is judged when that loan forms, and an operand a call stages in
//! its access roster is judged against the loans live at that invocation.

use arena::Handle;
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, CapturedPlace, CheckFacts, FlowStateFact, FlowStatementFact,
    StateBorrowFact,
};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use validation::CanonicalPlace;

use super::details::active_loan_detail;
use super::overlap::StatedOrderingPremise;
use super::place_access;
use crate::checks::ownership_occurrences::{StatementOccurrence, StatementStep};
use crate::labels::symbol_name;

/// Each loan live at the statement's entry whose last use is this statement,
/// with the last step that still holds it there.
pub(super) fn final_holds_in_statement(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: &FlowStatementFact,
    statement_node: &StatementNode,
    occurrences: &[StatementOccurrence],
) -> Vec<(Handle<BorrowLoanFact>, usize)> {
    let destination_root = match statement_node {
        StatementNode::Assignment(assignment) => {
            validation::canonical_place_from_expression_in_state(
                program,
                state.state_symbol,
                statement.statement_index,
                assignment.target,
            )
            .and_then(|place| place.root.as_symbol())
        }
        StatementNode::LocalData(_)
        | StatementNode::Expression(_)
        | StatementNode::Call(_)
        | StatementNode::Transition(_) => None,
        StatementNode::RootBinding(_)
        | StatementNode::Assembly(_)
        | StatementNode::AssemblyFact(_) => return Vec::new(),
    };
    facts
        .flow
        .borrow_loan_constraints(statement.entry_constraints)
        .filter_map(|handle| {
            let loan = facts.borrow.loans.get(handle);
            if loan.last_use_statement_index != statement.statement_index
                || !loan.owner_path.is_empty()
                || destination_root == Some(loan.owner_symbol)
            {
                return None;
            }
            occurrences
                .iter()
                .filter(|occurrence| {
                    matches!(&occurrence.step, StatementStep::Observe { place, .. }
                        if place.root.as_symbol() == Some(loan.owner_symbol))
                })
                .map(|occurrence| occurrence.held_until)
                .max()
                .map(|held_until| (handle, held_until))
        })
        .collect()
}

/// One evaluated observation and the loans live where its statement
/// evaluates it.
pub(super) struct StatementRead<'a> {
    pub(super) state: &'a FlowStateFact,
    pub(super) borrow_state: &'a StateBorrowFact,
    pub(super) statement: &'a FlowStatementFact,
    pub(super) place: &'a CanonicalPlace,
    /// Invalid for a statement call's receiver, which has no expression.
    pub(super) expression: ExpressionHandle,
    pub(super) active_loans: &'a [Handle<BorrowLoanFact>],
}

/// The authored operands the statement's calls stage in their own access
/// rosters, collected the first time an observation conflicts.
#[derive(Default)]
pub(super) struct StagedOperands {
    expressions: Option<Vec<ExpressionHandle>>,
}

impl StagedOperands {
    fn contains(&mut self, facts: &CheckFacts, read: &StatementRead<'_>) -> bool {
        if !read.expression.is_valid() {
            return false;
        }
        let statement_index = read.statement.statement_index;
        let expressions = self.expressions.get_or_insert_with(|| {
            let mut expressions = Vec::new();
            for call in facts
                .borrow
                .calls
                .span_or_empty(read.borrow_state.calls)
                .iter()
                .filter(|call| call.statement_index == statement_index)
            {
                expressions.extend(
                    facts
                        .borrow
                        .argument_accesses
                        .span_or_empty(call.accesses)
                        .iter()
                        .map(|access| access.expression),
                );
            }
            expressions
        });
        expressions.contains(&read.expression)
    }
}

pub(super) fn check_storage_read<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    read: StatementRead<'_>,
    staged_operands: &mut StagedOperands,
    premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    if read.active_loans.is_empty() {
        return;
    }
    // A completed rvalue has no preexisting storage to observe.
    let Some(root) = read.place.root.as_symbol() else {
        return;
    };
    let statement_index = read.statement.statement_index;
    let borrow = read
        .expression
        .is_valid()
        .then(|| program.expression_table.expression(read.expression))
        .and_then(|node| match node {
            ExpressionNode::Borrow(borrow) => Some(borrow.access),
            _ => None,
        });
    let kind = match borrow {
        Some(ReferenceAccess::Mutable) => BorrowAccessKind::Mutable,
        Some(ReferenceAccess::WriteOnly) => BorrowAccessKind::WriteOnly,
        Some(ReferenceAccess::Shared) | None => BorrowAccessKind::Read,
    };
    let verb = if borrow.is_some() { "borrows" } else { "reads" };
    let backings = place_access::place_backings(
        program,
        facts,
        read.state,
        statement_index,
        read.statement.entry_constraints,
        read.active_loans.iter().copied(),
        CapturedPlace {
            incoming_reference: Default::default(),
            root: facts::PlaceRoot::Symbol(root),
            segments: read.place.segments.clone(),
        },
    );
    let conflicts: Vec<_> = backings.as_ref().map_or_else(Vec::new, |backings| {
        read.active_loans
            .iter()
            .copied()
            .filter(|loan| {
                place_access::access_interferes(
                    program,
                    &facts.borrow,
                    backings,
                    &kind,
                    *loan,
                    premises,
                    bound_lookup,
                )
            })
            .collect()
    });
    if backings.is_some() && conflicts.is_empty() {
        return;
    }
    let forms_loan = read.expression.is_valid()
        && facts.borrow.loans.iter().any(|(handle, loan)| {
            loan.statement_index == statement_index
                && loan.formation_expression == read.expression
                && facts.borrow.state_owns_loan(read.borrow_state, handle)
        });
    if forms_loan || staged_operands.contains(facts, &read) {
        return;
    }
    let label = read.place.label(program);
    if backings.is_none() {
        diagnostics.push(Diagnostic::error(format!(
            "statement {statement_index} {verb} `{label}` without an exact retained backing for its loan",
        )));
        return;
    }
    for loan_handle in conflicts {
        let loan = facts.borrow.loans.get(loan_handle);
        diagnostics.push(Diagnostic::error(format!(
            "statement {statement_index} {verb} `{label}` while local borrow `{}` is still active ({})",
            symbol_name(program, loan.owner_symbol),
            active_loan_detail(read.state, facts, loan_handle, statement_index)
                .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
        )));
    }
}
