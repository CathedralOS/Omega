//! Exclusive-place operations and explicit state-jump subject access.
//! Ordinary machine arguments use the shared call compatibility checks.

use checked_trees::{BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

use super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
use super::super::statements::local_jump;

mod aliases;
mod footprint;
mod returned;

pub(in crate::checks::borrows) use footprint::{
    ReceiverSummaries, check_receiver_footprint_against_argument_loans,
};
pub(super) use footprint::{entered_subject_access, explicit_jump_data_access};

/// Whether `call` runs its target on the caller's implicit receiver.
///
/// An authored receiver operand records `has_receiver`. A named jump to a
/// sibling state spells none, yet a successor declaring `self` runs on the
/// caller's own `self` while it holds every loan the jump's arguments carry:
/// `transition { _ -> next(&mut self.count) }` is the invocation
/// `self.next(&mut self.count)` with the successor as callee, and the same
/// receiver judgments apply. Each judgment still requires the target's `self`
/// parameter, so a successor without one passes nothing.
fn call_passes_receiver(
    program: &TypedTrees,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> bool {
    call.has_receiver || local_jump(program, state_flow, call).is_some()
}

/// A compiler-owned operation borrows a checked place without invoking a
/// source machine. Retained alias ancestry supplies authority, not competing
/// loans; every other live overlapping loan must remain excluded.
pub(in crate::checks::borrows) fn check_exclusive_place_use<'p>(
    program: &'p TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    statement: &checked_trees::FlowStatementFact,
    expression: typed_trees::expression::ExpressionHandle,
    later_operand: typed_trees::expression::ExpressionHandle,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let Some(receiver) = returned::resolve(
        program,
        facts,
        state_flow,
        statement,
        expression,
        call_frames,
    ) else {
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
        if receiver.lineage.contains(&loan_handle) {
            continue;
        }
        let loan = facts.borrow.loans.get(loan_handle);
        if !captured_place_compatibility(
            program,
            &receiver.place,
            &BorrowAccessKind::Mutable,
            &CapturedPlace {
                root_symbol: loan.root_symbol,
                segments: facts.borrow.loan_segments(loan).to_vec(),
            },
            &loan.kind,
            stated_premises,
            bound_lookup,
        )
        .non_interfering
        {
            diagnostics.push(Diagnostic::error(format!(
                "exclusive operation receiver overlaps local borrow `{}` which is still active",
                program.symbols.name(loan.owner_symbol),
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
        ) || crate::flow::canonical_place_from_expression_in_state(
            program,
            state_flow.state_symbol,
            statement.statement_index,
            expression,
        )
        .is_none_or(|place| !matches!(place.root, facts::PlaceRoot::Symbol(_))))
    {
        let mut segments = arena::Arena::new();
        let mut accesses = arena::Arena::new();
        let span = crate::borrow::accesses::collect_call_argument_accesses(
            program,
            &mut segments,
            &mut accesses,
            &[later_operand],
            state_flow.state_symbol,
            statement.statement_index,
            state_flow.machine_symbol,
            symbols::SymbolHandle::default(),
        );
        for access in accesses.span_or_empty(span) {
            let place = CapturedPlace {
                root_symbol: access.root_symbol,
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
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    receiver: &CapturedPlace,
) -> bool {
    let Some(state) = crate::semantic::calls::find_state(program, state_flow.state_symbol) else {
        return false;
    };
    if receiver.root_symbol == state_flow.machine_symbol {
        return validation::receiver_allows_mutation(program, program.state_parameters(state));
    }
    // Writable-root facts retain local bindings; the declared reference access
    // still decides whether the referent can be exclusively borrowed.
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let typed_trees::statement::StatementNode::LocalData(local) = statement
            && local.symbol == receiver.root_symbol
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
        .any(|root| facts.borrow.writable_roots.get(root).symbol == receiver.root_symbol)
}

/// Older access rows represent a direct self field as a root. Rejoin it to
/// this caller's exact attached storage before comparing it with whole self;
/// a same-named field under another owner must not become an alias.
fn attached_place(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    mut place: CapturedPlace,
) -> CapturedPlace {
    if program.machine_states(machine).iter().any(|state| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self && parameter.symbol == place.root_symbol)
    }) {
        place.root_symbol = machine.symbol;
    } else if let Some(field) = validation::exact_attached_field(
        program,
        machine,
        place.root_symbol,
        program.symbols.name(place.root_symbol),
    ) {
        place.root_symbol = machine.symbol;
        place.segments.insert(
            0,
            facts::PlaceSegment::Field {
                symbol: field.symbol,
            },
        );
    }
    place
}
