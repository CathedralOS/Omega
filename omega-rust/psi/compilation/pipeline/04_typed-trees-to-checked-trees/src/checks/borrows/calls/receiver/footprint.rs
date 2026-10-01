//! A whole `&mut` receiver is judged by what the callee accesses through it,
//! not as one exclusive operand, so arguments of disjoint fields stay usable
//! beside it. Those accesses still happen while the callee holds every loan
//! its arguments retain through invocation: a shared argument freezes
//! mutation of the storage it views, and an exclusive one suspends that
//! branch of the receiver for the whole call. A receiver write that
//! interferes with any such loan breaks that freeze or suspension, and so
//! does a receiver read of storage an exclusive loan suspends, whether the
//! callee reads it before or after writing through the argument. Copied reads
//! and completed nested-call loans end before invocation and do not constrain
//! the callee. Projected receivers compare the whole exclusive child with
//! every argument in `check_receiver_conflicts`.

use checked_trees::{BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

use super::super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
use super::attached_place;
use crate::flow::{CanonicalPlace, StateMutationSummaryCache, StateReceiverReadCache};
use crate::labels::call_target_label;

/// The receiver summaries a footprint judgment reads.
pub(in crate::checks::borrows) struct ReceiverSummaries<'cache> {
    pub(in crate::checks::borrows) writes: &'cache StateMutationSummaryCache,
    pub(in crate::checks::borrows) reads: &'cache StateReceiverReadCache,
}

pub(in crate::checks::borrows) fn check_receiver_footprint_against_argument_loans<'p>(
    program: &'p TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    stated_premises: &[StatedOrderingPremise],
    summaries: ReceiverSummaries<'_>,
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
    let attached = |places: Vec<CanonicalPlace>| -> Vec<CapturedPlace> {
        places
            .into_iter()
            .filter_map(|place| match place.root {
                facts::PlaceRoot::Symbol(root_symbol) => Some(attached_place(
                    program,
                    machine,
                    CapturedPlace {
                        root_symbol,
                        segments: place.segments,
                    },
                )),
                _ => None,
            })
            .collect()
    };
    let retained_arguments: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .filter(|argument| argument.end_before_access.is_none())
        .collect();
    let writes = attached(crate::flow::call_receiver_write_accesses(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        &facts.borrow,
        call,
        summaries.writes,
    ));
    // Reads interfere only with exclusive loans, and most calls retain none.
    let reads = if retained_arguments
        .iter()
        .any(|argument| argument.kind.is_exclusive())
    {
        attached(crate::flow::call_receiver_read_accesses(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            call,
            summaries.reads,
        ))
    } else {
        Vec::new()
    };
    if writes.is_empty() && reads.is_empty() {
        return;
    }
    for argument in retained_arguments {
        let argument_place = attached_place(
            program,
            machine,
            CapturedPlace {
                root_symbol: argument.root_symbol,
                segments: facts.borrow.access_segments(argument).to_vec(),
            },
        );
        let mut interferes = |receiver_place: &CapturedPlace, receiver_access| {
            !captured_place_compatibility(
                program,
                receiver_place,
                receiver_access,
                &argument_place,
                &argument.kind,
                stated_premises,
                bound_lookup,
            )
            .non_interfering
        };
        let Some((receiver_access, receiver_place)) = writes
            .iter()
            .find(|write| interferes(write, &BorrowAccessKind::Mutable))
            .map(|write| ("writes", write))
            .or_else(|| {
                reads
                    .iter()
                    .find(|read| interferes(read, &BorrowAccessKind::Read))
                    .map(|read| ("reads", read))
            })
        else {
            continue;
        };
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` receives `{}` as {} while its mutable receiver {receiver_access} `{}`",
            call_target_label(program, call.target_symbol),
            attached_label(program, state_flow, &argument_place),
            match argument.kind {
                BorrowAccessKind::Read => "read-only",
                BorrowAccessKind::Mutable => "mutable",
                BorrowAccessKind::WriteOnly => "write-only",
            },
            attached_label(program, state_flow, receiver_place),
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
