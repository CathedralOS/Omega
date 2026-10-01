//! How much of a call target's run happens under loans its caller holds.
//!
//! A returning call keeps every loan its caller holds, including the loans its
//! own arguments retain, live until it returns, so the target's whole run
//! counts: its body, its calls and every state it jumps to. A named jump into
//! a sibling state instead ends the source state's run. The successor holds
//! the loans the jump's arguments carry until one of its own jumps stops
//! forwarding them. A further jump whose retained arguments are all rooted at
//! the receiver hands the next state fresh loans only, so the loans the
//! successor received end there and the next state may again reach the lent
//! storage through `self` (`pass/borrows/projected_mut_member_loan`).
//!
//! Receiver footprints are kept for both spans: the whole run, which a call
//! holds its argument loans across, and the held run, which a jump holds its
//! argument loans across.

use crate::flow::CanonicalPlace;
use crate::flow::normalize_attached_place_root;
use crate::semantic::calls::{CallSite, find_call_site, find_state_in_machine};
use checked_trees::{BorrowCallFact, BorrowFacts};
use symbols::SymbolHandle;

/// What part of a call target's run a loan the calling state received from
/// a jump still covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetRun {
    /// A returning call: the target's whole run, under every loan the caller
    /// holds.
    Whole,
    /// A jump forwarding a loan the source state may hold: the successor's
    /// held run.
    Held,
    /// A jump whose retained arguments are all fresh loans of the receiver:
    /// the source state's received loans end before the successor runs.
    Released,
}

/// What part of `call`'s target run the loans the calling state received from
/// a jump still cover. A jump's own argument loans cover its successor's held
/// run whether or not it also forwards received loans; a call's own argument
/// loans cover the whole run.
pub(crate) fn target_run(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    call: &BorrowCallFact,
) -> TargetRun {
    let jumps = find_state_in_machine(program, machine_symbol, call.target_symbol).is_some()
        && matches!(
            find_call_site(
                program,
                machine_symbol,
                state_symbol,
                call.statement_index,
                call.call_ordinal,
            ),
            Some(CallSite::TransitionNamed { .. })
        );
    if !jumps {
        return TargetRun::Whole;
    }
    // An argument rooted anywhere else (a reference parameter, a local view,
    // an unplaceable operand) may carry a loan the source received.
    let forwards = borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .filter(|access| access.end_before_access.is_none())
        .any(|access| {
            let mut root = CanonicalPlace {
                root: facts::PlaceRoot::Symbol(access.root_symbol),
                segments: Vec::new(),
            };
            normalize_attached_place_root(program, machine_symbol, state_symbol, &mut root);
            !rooted_at_receiver(program, machine_symbol, state_symbol, &root)
        });
    if forwards {
        TargetRun::Held
    } else {
        TargetRun::Released
    }
}

fn rooted_at_receiver(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    place: &CanonicalPlace,
) -> bool {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    root.is_valid()
        && (root == machine_symbol
            || find_state_in_machine(program, machine_symbol, state_symbol).is_some_and(|state| {
                program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.is_self && parameter.symbol == root)
            }))
}
