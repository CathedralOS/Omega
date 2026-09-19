//! Certificates cannot authenticate the roster that selected their subjects.
//! Recollect source facts, then join each flow invocation to its state-owned
//! call. These checked-only arenas retain construction identities; replay is
//! before any remapping, not a comparison between portable representations.
//!
//! Lexical preorder is not execution order: nested operands execute first and
//! short-circuited calls can have no flow row. Such calls retain the conservative
//! statement-entry check. Rebuild entry loans from source formation, expiry,
//! and reassignment, including derived loans without retained resource closure.
//! A forged activation/weakening plus a deleted certificate must not erase a
//! comparison. This replay does not create or restore resource authority.

use arena::{Arena, Handle, HandleSpan};
use checked_trees::{
    CheckFacts, FlowBorrowWeakeningReason, FlowConstraintKind, FlowConstraintRef, FlowStateFact,
};

pub(super) fn matches_source(program: &typed_trees::TypedTrees, facts: &CheckFacts) -> bool {
    let expected = crate::borrow::build_borrow_facts(program);
    // There are no call judgments to authenticate in a call-free program.
    // Other borrow/resource validators still own its formation and mutations.
    if expected.calls.is_empty()
        && facts.borrow.calls.is_empty()
        && facts.flow.control.calls.is_empty()
    {
        return true;
    }
    // Compare source-owned rows and handles, not certificate/resource outputs,
    // arena capacity or dummy storage. All captured places share this segment
    // arena, so checking only call rows would miss jointly forged loan places.
    if !expected.states.iter().eq(facts.borrow.states.iter())
        || !expected.calls.iter().eq(facts.borrow.calls.iter())
        || !expected
            .argument_accesses
            .iter()
            .eq(facts.borrow.argument_accesses.iter())
        || !expected
            .access_segments
            .iter()
            .eq(facts.borrow.access_segments.iter())
        || !expected
            .owner_segments
            .iter()
            .eq(facts.borrow.owner_segments.iter())
        || !expected.loans.iter().eq(facts.borrow.loans.iter())
        || !expected
            .writable_roots
            .iter()
            .eq(facts.borrow.writable_roots.iter())
        || expected.states.len() != facts.flow.control.states.len()
    {
        return false;
    }
    for (_, owner) in expected.states.iter() {
        let mut flows = facts.flow.control.states.iter().filter(|(_, flow)| {
            flow.machine_symbol == owner.machine_symbol && flow.state_symbol == owner.state_symbol
        });
        let Some((_, flow)) = flows.next() else {
            return false;
        };
        if flows.next().is_some() {
            return false;
        }
        let retained = facts.borrow.calls.span_or_empty(owner.calls);
        let Some(flow_calls) = facts.flow.control.calls.span(flow.calls) else {
            return false;
        };
        for (position, invocation) in flow_calls.iter().enumerate() {
            if flow_calls[..position].iter().any(|prior| {
                prior.statement_index == invocation.statement_index
                    && prior.call_ordinal == invocation.call_ordinal
            }) {
                return false;
            }
            let Some(offset) = retained.iter().position(|call| {
                call.statement_index == invocation.statement_index
                    && call.call_ordinal == invocation.call_ordinal
            }) else {
                return false;
            };
            let call = &retained[offset];
            if call.target_symbol != invocation.target_symbol
                || call.receiver_symbol != invocation.receiver_symbol
                || call.has_receiver != invocation.has_receiver
                || call.accesses != invocation.accesses
            {
                return false;
            }
            let handle = Handle::from_parts(
                owner.calls.start().arena_index() + offset as u32,
                owner.calls.start().generation(),
            );
            let Some(constraints) = facts
                .flow
                .contexts
                .constraint_refs
                .span(invocation.entry_constraints)
            else {
                return false;
            };
            // Earlier calls can remain in the constraint prefix. This
            // invocation's own exact identity must occur once.
            if constraints.iter().filter(|entry| {
                matches!(entry.kind, FlowConstraintKind::BorrowCall { call } if call == handle)
            }).count() != 1 { return false; }
        }
    }
    true
}

/// Called only after the source-owned borrow roster has matched. Use the
/// producer's source filters without its retained activation/weakening rows.
pub(super) fn matches_entry_loans(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
) -> bool {
    let Some(state) = crate::semantic_calls::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return false;
    };
    let Some(owner) = super::super::matching_borrow_state(facts, state_flow) else {
        return false;
    };
    if owner.calls.is_empty() {
        return true;
    }
    let mut calls = facts
        .borrow
        .calls
        .span_or_empty(owner.calls)
        .iter()
        .peekable();
    let mut loans = facts
        .borrow
        .loans
        .span_or_empty(owner.loans)
        .iter()
        .enumerate()
        .peekable();
    let mut constraints: Arena<FlowConstraintRef> = Arena::new();
    let mut weakenings = Arena::new();
    let mut active = HandleSpan::empty();
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        active = crate::flow::filter_expired_borrow_loans(
            &mut weakenings,
            &mut constraints,
            active,
            &facts.borrow,
            statement_index,
            FlowBorrowWeakeningReason::LastUseExpired,
        );
        while calls
            .peek()
            .is_some_and(|call| call.statement_index == statement_index)
        {
            let Some(call) = calls.next() else {
                return false;
            };
            let retained = super::call_borrow_constraints(call, state_flow, facts);
            if facts.flow.contexts.constraint_refs.span(retained).is_none() {
                return false;
            }
            let expected = constraints
                .span_or_empty(active)
                .iter()
                .filter_map(|entry| {
                    if let FlowConstraintKind::BorrowLoan { loan } = entry.kind {
                        Some(loan)
                    } else {
                        None
                    }
                });
            if !facts.flow.borrow_loan_constraints(retained).eq(expected) {
                return false;
            }
        }
        // RHS calls see the old carrier. Retirement and replacement formation
        // occur only afterward; expiry already happened before statement entry.
        active = crate::flow::filter_reassigned_borrow_loans(
            &mut weakenings,
            &mut constraints,
            active,
            &facts.borrow,
            program,
            state.symbol,
            statement_index,
            statement,
        );
        while loans
            .peek()
            .is_some_and(|(_, loan)| loan.statement_index == statement_index)
        {
            let Some((offset, _)) = loans.next() else {
                return false;
            };
            let loan = Handle::from_parts(
                owner.loans.start().arena_index() + offset as u32,
                owner.loans.start().generation(),
            );
            crate::flow::append_constraint_ref(
                &mut constraints,
                &mut active,
                FlowConstraintKind::BorrowLoan { loan },
            );
        }
    }
    calls.next().is_none() && loans.next().is_none()
}
