//! State-jump footprints preserve the loans carried into a successor.
//! Ordinary returning calls use explicit argument exclusivity, not a
//! receiver-specific relaxation based on which fields the callee accesses.

use checked_trees::{BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

use super::super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
use super::{aliases, attached_place};
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
    let Some(machine) = whole_mutable_receiver_machine(program, state_flow, call) else {
        return;
    };
    let data_access = explicit_jump_data_access(program, facts, state_flow, call);
    let attached = |places| attached_places(program, machine, places);
    let retained_arguments: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .enumerate()
        .filter(|(ordinal, argument)| {
            Some(*ordinal) != data_access && argument.end_before_access.is_none()
        })
        .map(|(_, argument)| argument)
        .collect();
    let entry_constraints = super::super::call_borrow_constraints(call, state_flow, facts);
    for argument in retained_arguments {
        let operand = CapturedPlace {
            root_symbol: argument.root_symbol,
            segments: facts.borrow.access_segments(argument).to_vec(),
        };
        let mut loan = CanonicalPlace {
            root: facts::PlaceRoot::Symbol(operand.root_symbol),
            segments: operand.segments.clone(),
        };
        crate::flow::normalize_attached_place_root(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &mut loan,
        );
        // Each loan has its own state-exit boundary. A shared footprint for
        // the whole argument roster would prolong dropped loans whenever an
        // unrelated argument survives.
        let writes = attached(crate::flow::call_receiver_write_accesses(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            call,
            summaries.writes,
            &loan,
        ));
        let reads = if argument.kind.is_exclusive() {
            attached(crate::flow::call_receiver_read_accesses(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                &facts.borrow,
                call,
                summaries.reads,
                Some(&loan),
            ))
        } else {
            Vec::new()
        };
        // A local reference operand lends the storage its own loan captured,
        // so an exclusive one suspends that storage, not the local's name.
        // Only reads need the rejoined place: a receiver write to it already
        // rejects against the live local loan in `check_call_mutation_borrows`,
        // which judges writes and not reads.
        let lent_place = if reads.is_empty() || !argument.kind.is_exclusive() {
            None
        } else {
            aliases::resolve_place(
                program,
                facts,
                state_flow,
                entry_constraints,
                operand.clone(),
            )
            .filter(|resolved| resolved.local_loan.is_valid())
            .map(|resolved| attached_place(program, machine, resolved.place))
        };
        let argument_place = attached_place(program, machine, operand);
        let mut interferes =
            |receiver_place: &CapturedPlace, receiver_access, argument_place: &CapturedPlace| {
                !captured_place_compatibility(
                    program,
                    receiver_place,
                    receiver_access,
                    argument_place,
                    &argument.kind,
                    stated_premises,
                    bound_lookup,
                )
                .non_interfering
            };
        let read_argument_place = lent_place.as_ref().unwrap_or(&argument_place);
        let Some((receiver_access, receiver_place, argument_place)) = writes
            .iter()
            .find(|write| interferes(write, &BorrowAccessKind::Mutable, &argument_place))
            .map(|write| ("writes", write, &argument_place))
            .or_else(|| {
                reads
                    .iter()
                    .find(|read| interferes(read, &BorrowAccessKind::Read, read_argument_place))
                    .map(|read| ("reads", read, read_argument_place))
            })
        else {
            continue;
        };
        diagnostics.push(Diagnostic::error(format!(
            "state `{}` receives `{}` as {} while its mutable receiver {receiver_access} `{}`",
            call_target_label(program, call.target_symbol),
            attached_label(program, state_flow, argument_place),
            match argument.kind {
                BorrowAccessKind::Read => "read-only",
                BorrowAccessKind::Mutable => "mutable",
                BorrowAccessKind::WriteOnly => "write-only",
            },
            attached_label(program, state_flow, receiver_place),
        )));
    }
}

/// A local successor receives its data parameter together with the child loans
/// it keeps alive. The parent is judged by that successor's footprint, just as
/// the authored receiver is; it is not a second independent loan of every field.
/// Keep the original access roster and its coordinates for operand evaluation
/// and independent replay. Only one exact retained parent access can use this
/// judgment, and ordinary returning calls retain their operand exclusivity.
pub(in crate::checks::borrows::calls) fn explicit_jump_data_access(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> Option<usize> {
    super::local_jump(program, state_flow, call)?;
    let machine = whole_mutable_receiver_machine(program, state_flow, call)?;
    retained_subject_access(program, facts, state_flow, call, machine)
}

/// The same access, for a call of ANY form.
///
/// A receiver's loan is taken when the call is ENTERED, after its operands are
/// evaluated, so `self.bump(self.bump(3))` never holds two at once. The
/// explicit subject is an argument, so the roster holds the outer call's
/// subject beside the inner call's and the pairwise rule sees two mutable
/// names. This names the outer one, and the caller may only use it to excuse a
/// partner that already COMPLETED -- a genuine second live loan, as in
/// `Main::bump(main, main)`, keeps its refusal.
pub(in crate::checks::borrows::calls) fn entered_subject_access(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> Option<usize> {
    // The callee names which of ITS parameters is the subject; the caller's
    // argument in that position is the place to find in the roster. The jump
    // form reads the parent off a receiver expression instead, which a static
    // call does not have.
    let target = crate::semantic::calls::find_state(program, call.target_symbol)?;
    let parameters = program.state_parameters(target);
    let callee = program.machine_holding_state(call.target_symbol)?;
    let position = program.data_binding_position(callee.attached_data_symbol, parameters)?;
    if parameters[position].is_self {
        return None;
    }
    let site = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
    // Only a roster that corresponds one-to-one with the parameters locates
    // the subject by position; anything else keeps its ordinary exclusivity.
    if arguments.len() != parameters.len() {
        return None;
    }
    let parent = crate::flow::canonical_place_from_expression(program, *arguments.get(position)?)?;
    let facts::PlaceRoot::Symbol(root_symbol) = parent.root else {
        return None;
    };
    let caller = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)?;
    retained_access_for_parent(
        program,
        facts,
        call,
        caller,
        attached_place(
            program,
            caller,
            CapturedPlace {
                root_symbol,
                segments: parent.segments,
            },
        ),
    )
}

/// The one RETAINED mutable access naming `call`'s subject parent, if exactly
/// one does. A completed access is never it: the filter below drops them.
fn retained_subject_access(
    program: &TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    machine: &typed_trees::machine::Machine,
) -> Option<usize> {
    let target = crate::semantic::calls::find_state(program, call.target_symbol)?;
    let parameter = program.data_binding_parameter(machine, program.state_parameters(target))?;
    if parameter.is_self {
        return None;
    }
    let site = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parent = crate::flow::canonical_receiver_place_for_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        &site,
        call.statement_index,
    )?;
    let facts::PlaceRoot::Symbol(root_symbol) = parent.root else {
        return None;
    };
    // Access rows and expression places may spell the authored self root
    // differently. Compare them in the same attached-storage coordinates
    // used by the footprint judgment; ordinary named parameter roots retain
    // their distinct identities.
    retained_access_for_parent(
        program,
        facts,
        call,
        machine,
        attached_place(
            program,
            machine,
            CapturedPlace {
                root_symbol,
                segments: parent.segments,
            },
        ),
    )
}

/// The one RETAINED mutable access of `parent`, if exactly one names it.
///
/// Access rows and expression places may spell the authored self root
/// differently, so both sides are compared in the attached-storage
/// coordinates the footprint judgment uses; ordinary named parameter roots
/// retain their distinct identities. A completed access is never the answer:
/// it no longer holds the loan the caller is reasoning about.
fn retained_access_for_parent(
    program: &TypedTrees,
    facts: &CheckFacts,
    call: &BorrowCallFact,
    machine: &typed_trees::machine::Machine,
    parent: CapturedPlace,
) -> Option<usize> {
    let mut matching = facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .enumerate()
        .filter(|(_, access)| {
            if access.kind != BorrowAccessKind::Mutable || access.end_before_access.is_some() {
                return false;
            }
            let place = attached_place(
                program,
                machine,
                CapturedPlace {
                    root_symbol: access.root_symbol,
                    segments: facts.borrow.access_segments(access).to_vec(),
                },
            );
            place.root_symbol == parent.root_symbol && place.segments == parent.segments
        });
    let (ordinal, _) = matching.next()?;
    matching.next().is_none().then_some(ordinal)
}

/// The caller's machine when `call` passes a whole `&mut` receiver: the one
/// receiver form judged by its footprint instead of as a single operand.
fn whole_mutable_receiver_machine<'p>(
    program: &'p TypedTrees,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> Option<&'p typed_trees::machine::Machine> {
    if !super::call_passes_receiver(program, state_flow, call)
        || !receiver_is_mutable_reference(
            program,
            call.target_symbol,
            super::local_jump(program, state_flow, call).is_some(),
        )
    {
        return None;
    }
    let machine = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol)?;
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
    let Some(CanonicalPlace {
        root: facts::PlaceRoot::Symbol(root_symbol),
        segments,
    }) = receiver
    else {
        return None;
    };
    attached_place(
        program,
        machine,
        CapturedPlace {
            root_symbol,
            segments,
        },
    )
    .segments
    .is_empty()
    .then_some(machine)
}

/// Summarized receiver places rejoined to the caller's attached storage.
fn attached_places(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    places: Vec<CanonicalPlace>,
) -> Vec<CapturedPlace> {
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
    explicit_data: bool,
) -> bool {
    crate::semantic::calls::call_target_parameters(program, target_symbol).is_some_and(
        |parameters| {
            let data_parameter = explicit_data
                .then(|| crate::proof::contract_target_from_state_symbol(program, target_symbol))
                .flatten()
                .and_then(|(machine, _)| crate::lookup::machine_by_symbol(program, machine))
                .and_then(|machine| program.data_binding_parameter(machine, parameters));
            parameters.iter().any(|parameter| {
                (parameter.is_self
                    || data_parameter.is_some_and(|data| data.symbol == parameter.symbol))
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
