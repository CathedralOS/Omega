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
//! Every carrier read is checked at its evaluated occurrence, including a
//! reference operand later staged by a call. The completed operand separately
//! holds its captured referent through invocation; that hold cannot authorize
//! the earlier carrier read or erase a loan suspending its descriptor slot.

use arena::Handle;
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, CapturedPlace, CheckFacts, FlowStateFact, FlowStatementFact,
};
use diagnostics::Diagnostic;
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
    ends_source_path: bool,
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
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => return Vec::new(),
    };
    facts
        .flow
        .borrow_loan_constraints(statement.entry_constraints)
        .filter_map(|handle| {
            let loan = facts.borrow.loans.get(handle);
            if ends_source_path {
                let (_, local) = program.state_local_declaration(
                    program.state_by_symbol(state.state_symbol)?,
                    loan.owner_symbol,
                )?;
                if !matches!(
                    program
                        .type_reference_table
                        .type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    return None;
                }
            }
            if (!ends_source_path && loan.last_use_statement_index != statement.statement_index)
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

/// A completed guard or earlier operand can end a local observation before
/// an ordinary returning call, not just at a state-scope exit. Reuse the
/// evaluated occurrence order: an operand held through this invocation ends
/// at its completion and therefore remains live during it. Missing/ambiguous
/// correspondence and carried reference leaves never supply this release.
pub(super) fn loans_ended_before_call(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    flow: &FlowStateFact,
    call: &checked_trees::BorrowCallFact,
    boundary: Option<&super::calls::InvocationBoundary>,
) -> Vec<Handle<BorrowLoanFact>> {
    let Some(boundary) = boundary else {
        return Vec::new();
    };
    let Some((_, state)) =
        validation::machine_state_by_symbols(program, flow.machine_symbol, flow.state_symbol)
    else {
        return Vec::new();
    };
    let Some(statement) = facts
        .flow
        .control
        .statements
        .span_or_empty(flow.statements)
        .iter()
        .find(|statement| statement.statement_index == call.statement_index)
    else {
        return Vec::new();
    };
    let Some(node) = program
        .statement_table
        .statement_at(state, call.statement_index)
    else {
        return Vec::new();
    };
    final_holds_in_statement(
        program,
        facts,
        flow,
        statement,
        node,
        &boundary.occurrences,
        boundary.ends_source_path,
    )
    .into_iter()
    .filter_map(|(loan, held_until)| (held_until < boundary.completion).then_some(loan))
    .collect()
}

/// One evaluated observation and the loans live where its statement
/// evaluates it.
pub(super) struct StatementRead<'a> {
    pub(super) state: &'a FlowStateFact,
    pub(super) statement: &'a FlowStatementFact,
    pub(super) place: &'a CanonicalPlace,
    /// Invalid for a statement call's receiver, which has no expression.
    pub(super) expression: ExpressionHandle,
    pub(super) active_loans: &'a [Handle<BorrowLoanFact>],
}

/// An actual transferred view value names its descriptor binding, not a read
/// through that descriptor. Projected slots still read their complete receiver
/// path, and contextual scalar observations still read the referee. Ordinary
/// ownership/liveness and the eventual invocation's permission judgment remain
/// independent; this classification creates neither a loan nor early release.
fn transfers_descriptor_value(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    read: &StatementRead<'_>,
) -> bool {
    if !read.expression.is_valid()
        || !read.place.segments.is_empty()
        || matches!(
            program.expression_table.expression(read.expression),
            ExpressionNode::Borrow(_)
        )
    {
        return false;
    }
    let Some(source) = validation::canonical_place_from_expression_in_state(
        program,
        read.state.state_symbol,
        read.statement.statement_index,
        read.expression,
    ) else {
        return false;
    };
    if source.root != read.place.root || !source.segments.is_empty() {
        return false;
    }
    let Some(declared) = validation::canonical_place_type_reference(
        program,
        read.state.state_symbol,
        read.statement.statement_index,
        &source,
    ) else {
        return false;
    };
    if !crate::flow::operand_binds_live_view(program, declared) {
        return false;
    }
    // These are the producer's actual value consumers, not a roster of source
    // expressions a call happened to mention. Never descend into a matching
    // scalar observation's backing: that is precisely its referee read.
    let control = &facts.flow.control;
    let mut pending = Vec::new();
    pending.extend(
        read.statement
            .stored_value
            .is_valid()
            .then_some(read.statement.stored_value),
    );
    for call in control.calls.span_or_empty(read.state.calls) {
        if call.statement_index == read.statement.statement_index {
            pending.extend(call.operands.handles());
        }
    }
    for outgoing in control.outgoing.span_or_empty(read.state.outgoing) {
        if outgoing.statement_index == read.statement.statement_index
            && outgoing.returned_value.is_valid()
        {
            pending.push(outgoing.returned_value);
        }
    }
    for observation in control
        .value_observations
        .span_or_empty(read.statement.value_observations)
    {
        if observation.result.is_valid() {
            pending.push(observation.result);
        }
    }
    let mut visited = Vec::new();
    let mut found = false;
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        if !control.operands.is_valid(handle) {
            return false;
        }
        let value = control.operands.get(handle);
        if value.state_symbol != read.state.state_symbol
            || value.statement_index != read.statement.statement_index
        {
            continue;
        }
        if value.expression == read.expression {
            // A loaded inner descriptor can still be reference-typed. It is
            // not this binding's value: retain every declared reference shell,
            // access and qualification, and require a direct capture without
            // an evaluated receiver/backing wrapper. Such loads keep the
            // ordinary storage-read judgment even when their final type is &T.
            if value.backing.is_valid()
                || !crate::flow::operand_binds_live_view(program, value.type_reference)
                || program.normalized_type_identity(value.type_reference)
                    != program.normalized_type_identity(declared)
            {
                return false;
            }
            found = true;
            continue;
        }
        match value.completion {
            checked_trees::FlowOperandCompletion::Constructed(children) => {
                let Some(children) = control.operand_children.span(children) else {
                    return false;
                };
                pending.extend(children.iter().map(|child| child.value));
            }
            checked_trees::FlowOperandCompletion::Selected(alternatives) => {
                let Some(alternatives) = control.operand_alternatives.span(alternatives) else {
                    return false;
                };
                if alternatives.is_empty()
                    || alternatives
                        .iter()
                        .any(|alternative| !alternative.source_arm.is_valid())
                {
                    return false;
                }
                pending.extend(alternatives.iter().map(|alternative| alternative.value));
            }
            checked_trees::FlowOperandCompletion::Observation
            | checked_trees::FlowOperandCompletion::Copied { .. } => {}
        }
    }
    found
}

pub(super) fn check_storage_read<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    read: StatementRead<'_>,
    premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    if read.active_loans.is_empty() || transfers_descriptor_value(program, facts, &read) {
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
    let kind = borrow.map_or(BorrowAccessKind::Read, BorrowAccessKind::from);
    let verb = if borrow.is_some() { "borrows" } else { "reads" };
    let backings = place_access::place_backings(
        program,
        facts,
        read.state,
        statement_index,
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
