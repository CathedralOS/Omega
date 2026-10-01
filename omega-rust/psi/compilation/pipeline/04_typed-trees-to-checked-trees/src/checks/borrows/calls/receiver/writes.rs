//! A whole `&mut` receiver is judged by what the callee writes through it,
//! not as one exclusive operand, so arguments of disjoint fields stay usable
//! beside it. Those writes still happen while the callee holds every loan its
//! arguments retain through invocation: a shared argument freezes mutation of
//! the storage it views, and an exclusive one suspends it. A receiver write
//! that interferes with such a loan breaks that freeze or suspension. Copied
//! reads and completed nested-call loans end before invocation and do not
//! constrain the callee. Projected receivers compare the whole exclusive
//! child with every argument in `check_receiver_conflicts`.

use checked_trees::{BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

use super::super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
use super::attached_place;
use crate::flow::StateMutationSummaryCache;
use crate::labels::call_target_label;

pub(in crate::checks::borrows) fn check_receiver_writes_against_argument_loans<'p>(
    program: &'p TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    stated_premises: &[StatedOrderingPremise],
    summary_cache: &StateMutationSummaryCache,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    if !call.has_receiver || !receiver_is_mutable_reference(program, call.target_symbol) {
        return;
    }
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return;
    };
    let receiver = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )
    .and_then(|site| {
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &site,
            call.statement_index,
        )
    });
    let Some(crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(root_symbol),
        segments,
    }) = receiver
    else {
        return;
    };
    if !attached_place(
        program,
        machine,
        CapturedPlace {
            root_symbol,
            segments,
        },
    )
    .segments
    .is_empty()
    {
        return;
    }
    let writes: Vec<_> = crate::flow::call_receiver_write_accesses(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        &facts.borrow,
        call,
        summary_cache,
    )
    .into_iter()
    .filter_map(|write| match write.root {
        facts::PlaceRoot::Symbol(root_symbol) => Some(attached_place(
            program,
            machine,
            CapturedPlace {
                root_symbol,
                segments: write.segments,
            },
        )),
        _ => None,
    })
    .collect();
    if writes.is_empty() {
        return;
    }
    for argument in facts.borrow.argument_accesses.span_or_empty(call.accesses) {
        if argument.end_before_access.is_some() {
            continue;
        }
        let argument_place = attached_place(
            program,
            machine,
            CapturedPlace {
                root_symbol: argument.root_symbol,
                segments: facts.borrow.access_segments(argument).to_vec(),
            },
        );
        let Some(write_place) = writes.iter().find(|write_place| {
            !captured_place_compatibility(
                program,
                write_place,
                &BorrowAccessKind::Mutable,
                &argument_place,
                &argument.kind,
                stated_premises,
                bound_lookup,
            )
            .non_interfering
        }) else {
            continue;
        };
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` receives `{}` as {} while its mutable receiver writes `{}`",
            call_target_label(program, call.target_symbol),
            attached_label(program, state_flow, &argument_place),
            match argument.kind {
                BorrowAccessKind::Read => "read-only",
                BorrowAccessKind::Mutable => "mutable",
                BorrowAccessKind::WriteOnly => "write-only",
            },
            attached_label(program, state_flow, write_place),
        )));
    }
}

/// Attached storage reads as the caller's `self` path in diagnostics.
fn attached_label(
    program: &TypedTrees,
    state_flow: &FlowStateFact,
    place: &CapturedPlace,
) -> String {
    let self_parameter = (place.root_symbol == state_flow.machine_symbol)
        .then(|| crate::semantic::calls::find_state(program, state_flow.state_symbol))
        .flatten()
        .and_then(|state| {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.is_self)
        });
    facts::canonical_place_label_from_parts(
        program,
        facts::PlaceRoot::Symbol(
            self_parameter.map_or(place.root_symbol, |parameter| parameter.symbol),
        ),
        &place.segments,
    )
}

fn receiver_is_mutable_reference(
    program: &TypedTrees,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    crate::semantic::calls::call_target_parameters(program, target_symbol).is_some_and(
        |parameters| {
            parameters.iter().any(|parameter| {
                parameter.is_self
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        TypeReferenceNode::Reference {
                            access: ReferenceAccess::Mutable,
                            ..
                        }
                    )
            })
        },
    )
}
