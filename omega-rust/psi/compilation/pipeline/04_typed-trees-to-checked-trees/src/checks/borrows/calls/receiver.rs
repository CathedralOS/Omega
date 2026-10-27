//! Exclusive-place operations and explicit state-jump subject access.
//! Ordinary machine arguments use the shared call compatibility checks.

use checked_trees::{BorrowAccessKind, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use typed_trees::types::TypeReferenceNode;

use super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
use super::super::statements::local_jump;

use super::super::{access_sources, aliases, place_access};
mod footprint;

pub(super) use footprint::explicit_jump_data_access;
pub(in crate::checks::borrows) use footprint::{
    ReceiverSummaries, check_receiver_footprint_against_argument_loans,
};

/// A compiler-owned operation borrows a checked place without invoking a
/// source machine. Retained alias ancestry supplies authority, not competing
/// loans; every other live overlapping loan must remain excluded.
pub(in crate::checks::borrows) fn check_exclusive_place_use<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement: &checked_trees::FlowStatementFact,
    expression: typed_trees::expression::ExpressionHandle,
    later_operand: typed_trees::expression::ExpressionHandle,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let Some(receiver) = access_sources::resolve(program, facts, state_flow, statement, expression)
    else {
        diagnostics.push(Diagnostic::error(
            "exclusive operation requires an exact receiver source and available loan authority",
        ));
        return;
    };
    if !receiver_is_writable(
        program,
        facts,
        state_flow,
        statement.entry_constraints,
        &receiver.place,
    ) {
        diagnostics.push(Diagnostic::error(
            "exclusive operation receiver is not writable in this state",
        ));
        return;
    }
    for loan_handle in facts
        .flow
        .borrow_loan_constraints(statement.entry_constraints)
    {
        if place_access::access_interferes(
            program,
            &facts.borrow,
            std::slice::from_ref(&receiver),
            &BorrowAccessKind::Mutable,
            loan_handle,
            stated_premises,
            bound_lookup,
        ) {
            diagnostics.push(Diagnostic::error(format!(
                "exclusive operation receiver overlaps local borrow `{}` which is still active",
                program
                    .symbols
                    .name(facts.borrow.loans.get(loan_handle).owner_symbol),
            )));
        }
    }
    // A computed reference or explicit borrow has no local loan owner. It remains held
    // across the later operand: every access to overlapping backing storage
    // conflicts, including access through an ancestor alias. Stored receivers
    // already have ordinary loan liveness and call-conflict checks.
    if later_operand.is_valid()
        && (matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Borrow(_)
        ) || validation::canonical_place_from_expression_in_state(
            program,
            state_flow.state_symbol,
            statement.statement_index,
            expression,
        )
        .is_none_or(|place| !matches!(place.root, facts::PlaceRoot::Symbol(_))))
    {
        let mut segments = arena::Arena::new();
        let mut accesses = arena::Arena::new();
        let span = crate::borrow::accesses::collect_argument_accesses(
            program,
            &mut segments,
            &mut accesses,
            &[later_operand],
            state_flow.state_symbol,
            statement.statement_index,
            state_flow.machine_symbol,
            symbols::SymbolHandle::default(),
            crate::borrow::accesses::ArgumentAccessSchedule::Invocation,
        );
        for access in accesses.span_or_empty(span) {
            let place = CapturedPlace {
                incoming_reference: Default::default(),
                root: access.root,
                segments: segments.span_or_empty(access.segments).to_vec(),
            };
            let Some(access) = aliases::resolve_place(
                program,
                facts,
                state_flow,
                statement.entry_constraints,
                place,
            ) else {
                diagnostics.push(Diagnostic::error(
                    "exclusive receiver result requires an exact later operand loan origin",
                ));
                continue;
            };
            if !captured_place_compatibility(
                program,
                &receiver.place,
                &BorrowAccessKind::Mutable,
                &access.place,
                &BorrowAccessKind::Read,
                stated_premises,
                bound_lookup,
            )
            .non_interfering
            {
                diagnostics.push(Diagnostic::error(
                    "later operand accesses storage held by the exclusive receiver result",
                ));
            }
        }
    }
}

fn receiver_is_writable(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    receiver: &CapturedPlace,
) -> bool {
    let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
        return false;
    };
    if receiver.root == facts::PlaceRoot::Symbol(state_flow.machine_symbol) {
        return false;
    }
    // Writable-root facts retain local bindings; the declared reference access
    // still decides whether the referent can be exclusively borrowed.
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let Some(local) = statement.as_local_data()
            && facts::PlaceRoot::Symbol(local.symbol) == receiver.root
        {
            return match program
                .type_reference_table
                .type_reference(local.type_reference)
            {
                TypeReferenceNode::Reference { access, .. } => access.is_exclusive(),
                _ => local.is_mutable,
            };
        }
    }
    facts
        .flow
        .borrow_writable_root_constraints(entry_constraints)
        .any(|root| {
            facts::PlaceRoot::Symbol(facts.borrow.writable_roots.get(root).symbol) == receiver.root
        })
}
