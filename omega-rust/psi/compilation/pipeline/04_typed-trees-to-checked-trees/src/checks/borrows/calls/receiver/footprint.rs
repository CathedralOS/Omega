//! State-jump footprints preserve the loans carried into a successor.
//! Ordinary returning calls use explicit argument exclusivity, not a
//! receiver-specific relaxation based on which fields the callee accesses.

use validation::CanonicalPlace;

use checked_trees::{BorrowAccessKind, BorrowCallFact, CapturedPlace, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::types::TypeReferenceNode;

use super::super::super::overlap::{StatedOrderingPremise, captured_place_compatibility};
use super::aliases;
use crate::flow::{StateMutationSummaryCache, StateReceiverReadCache};
use crate::labels::call_target_label;

/// The receiver summaries a footprint judgment reads.
pub(in crate::checks::borrows) struct ReceiverSummaries<'cache> {
    pub(in crate::checks::borrows) writes: &'cache StateMutationSummaryCache,
    pub(in crate::checks::borrows) reads: &'cache StateReceiverReadCache,
}

pub(in crate::checks::borrows) fn check_receiver_footprint_against_argument_loans<'p>(
    program: &'p dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    stated_premises: &[StatedOrderingPremise],
    summaries: ReceiverSummaries<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    if whole_mutable_jump_machine(program, state_flow, call).is_none() {
        return;
    }
    let data_access = explicit_jump_data_access(program, facts, state_flow, call);
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
    let entry_constraints = super::super::call_borrow_constraints(program, call, state_flow, facts);
    for argument in retained_arguments {
        let operand = CapturedPlace {
            incoming_reference: Default::default(),
            root: argument.root,
            segments: facts.borrow.access_segments(argument).to_vec(),
        };
        let loan = CanonicalPlace {
            root: operand.root,
            segments: operand.segments.clone(),
        };
        // Each loan has its own state-exit boundary. A shared footprint for
        // the whole argument roster would prolong dropped loans whenever an
        // unrelated argument survives.
        let writes = captured_places(crate::flow::call_receiver_write_accesses(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            call,
            summaries.writes,
            &loan,
        ));
        let reads = if argument.kind.is_exclusive() {
            captured_places(crate::flow::call_receiver_read_accesses(
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
            .map(|resolved| resolved.place)
        };
        let argument_place = operand;
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
            place_label(program, argument_place),
            match argument.kind {
                BorrowAccessKind::Read => "read-only",
                BorrowAccessKind::Mutable => "mutable",
                BorrowAccessKind::WriteOnly => "write-only",
            },
            place_label(program, receiver_place),
        )));
    }
}

/// A local successor receives its data parameter together with the child loans
/// it keeps alive. The parent is judged by that successor's footprint;
/// it is not a second independent loan of every field.
/// Keep the original access roster and its coordinates for operand evaluation
/// and independent replay. Only one exact retained parent access can use this
/// judgment, and ordinary returning calls retain their operand exclusivity.
pub(in crate::checks::borrows::calls) fn explicit_jump_data_access(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> Option<usize> {
    super::local_jump(program, state_flow, call)?;
    let machine = whole_mutable_jump_machine(program, state_flow, call)?;
    retained_subject_access(program, facts, state_flow, call, machine)
}

/// The one RETAINED mutable access naming `call`'s subject parent, if exactly
/// one does. A completed access is never it: the filter below drops them.
fn retained_subject_access(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    machine: &typed_trees::machine::Machine,
) -> Option<usize> {
    let target = program.state_by_symbol(call.target_symbol)?;
    program.data_binding_parameter(machine, program.state_parameters(target))?;
    let site = validation::find_call_site(
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
    let root = parent.root.as_symbol()?;
    let mut matching = facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .enumerate()
        .filter(|(_, access)| {
            access.kind == BorrowAccessKind::Mutable
                && access.end_before_access.is_none()
                && access.root == facts::PlaceRoot::Symbol(root)
                && facts.borrow.access_segments(access) == parent.segments
        });
    let (ordinal, _) = matching.next()?;
    matching.next().is_none().then_some(ordinal)
}

/// Only a local state jump transfers the current activation's data parent.
/// Returning calls establish ordinary loans, independent of callee footprints.
fn whole_mutable_jump_machine<'p>(
    program: &'p dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
) -> Option<&'p typed_trees::machine::Machine> {
    super::local_jump(program, state_flow, call)?;
    let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
    let target = program.state_by_symbol(call.target_symbol)?;
    let parameter = program.data_binding_parameter(machine, program.state_parameters(target))?;
    if !matches!(
        program
            .type_reference_table
            .type_reference(parameter.type_reference),
        TypeReferenceNode::Reference {
            access: ReferenceAccess::Mutable,
            ..
        }
    ) {
        return None;
    }
    let receiver = validation::find_call_site(
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
        root: facts::PlaceRoot::Symbol(_),
        segments,
    }) = receiver
    else {
        return None;
    };
    segments.is_empty().then_some(machine)
}

/// Summaries already use the caller's explicit storage coordinates, including
/// computed backing. Projection does not require a named declaration root.
fn captured_places(places: Vec<CanonicalPlace>) -> Vec<CapturedPlace> {
    places
        .into_iter()
        .filter_map(|place| {
            place.root.is_valid().then_some(CapturedPlace {
                incoming_reference: Default::default(),
                root: place.root,
                segments: place.segments,
            })
        })
        .collect()
}

fn place_label(program: &dyn ::validation::CheckingInput, place: &CapturedPlace) -> String {
    facts::canonical_place_label_from_parts(program, place.root, &place.segments)
}
