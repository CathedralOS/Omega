//! A whole `&mut` receiver is judged by what the callee accesses through it,
//! not as one exclusive operand, so arguments of disjoint fields stay usable
//! beside it. Those accesses still happen while the callee holds every loan
//! its arguments retain through invocation: a shared argument freezes
//! mutation of the storage it views, and an exclusive one suspends that
//! branch of the receiver for the whole call. A receiver write that
//! interferes with any such loan breaks that freeze or suspension, and so
//! does a receiver read of storage an exclusive loan suspends, whether the
//! callee reads it before or after writing through the argument; an exclusive
//! local reference operand suspends the storage its own loan captured. Copied
//! reads and completed nested-call loans end before invocation and do not
//! constrain the callee. Projected receivers compare the whole exclusive
//! child with every argument in `check_receiver_conflicts`. A named jump into
//! a `&mut self` sibling state is judged here too, with the successor as
//! callee and the caller's `self` as its receiver: the arguments' loans stay
//! live until that individual loan is no longer forwarded by a state jump.
//!
//! The loans live around a call that returns to its caller constrain the
//! callee the same way, whether or not an operand names them. A receiver
//! write against any of them rejects in `check_call_mutation_borrows`; a
//! receiver read of storage a live exclusive loan suspends rejects here
//! (`check_receiver_reads_against_live_loans`).

use arena::Handle;
use checked_trees::{
    BorrowAccessKind, BorrowCallFact, BorrowLoanFact, CapturedPlace, CheckFacts, FlowStateFact,
};
use diagnostics::Diagnostic;
use language_semantics::ReferenceAccess;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

use super::super::super::details::active_loan_detail;
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

/// `live_loans` are the loans live at `call`'s statement entry. A live
/// exclusive loan suspends the storage it captured until its last use, so a
/// callee reading any of it through the receiver observes storage that the
/// loan's own path may rewrite after the call returns, although no operand of
/// the call names the loan (`let view: &mut u64 = &mut self.count;
/// self.peek_count(); view = 3;`). A shared loan freezes only mutation, which
/// the callee's writes answer in `check_call_mutation_borrows`. A loan this
/// call lends through a retained exclusive argument is judged instead by
/// `check_receiver_footprint_against_argument_loans`, against the storage the
/// loan captured, so each conflict reports once.
///
/// Only a call that returns to its caller is judged here. A named jump's
/// successor runs after the source state's scope exit, so the loans it can
/// observe are the ones the jump's arguments carry, which the argument
/// judgment covers. A source-state loan live at the jump has ended by then;
/// the write judgment insists on replayed closure evidence for that release,
/// which views derived through a helper (`self.rooms.as_mut_slice()`) lack,
/// and that stricter release here would reject a successor that reads storage
/// whose view the jump's own guard used last
/// (`pass/text/runtime_slice_indexed_string_guard_exit`).
pub(in crate::checks::borrows) fn check_receiver_reads_against_live_loans<'p>(
    program: &'p TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    live_loans: &[Handle<BorrowLoanFact>],
    stated_premises: &[StatedOrderingPremise],
    reads: &StateReceiverReadCache,
    diagnostics: &mut Vec<Diagnostic>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let is_exclusive =
        |handle: &Handle<BorrowLoanFact>| facts.borrow.loans.get(*handle).kind.is_exclusive();
    if !returns_through_mutable_receiver(program, call) || !live_loans.iter().any(is_exclusive) {
        return;
    }
    let Some(machine) = whole_mutable_receiver_machine(program, state_flow, call) else {
        return;
    };
    let entry_constraints = super::super::call_borrow_constraints(call, state_flow, facts);
    let lent_loans: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(call.accesses)
        .iter()
        .filter(|argument| argument.kind.is_exclusive() && argument.end_before_access.is_none())
        .filter_map(|argument| {
            aliases::resolve_place(
                program,
                facts,
                state_flow,
                entry_constraints,
                CapturedPlace {
                    root_symbol: argument.root_symbol,
                    segments: facts.borrow.access_segments(argument).to_vec(),
                },
            )
        })
        .map(|resolved| resolved.local_loan)
        .filter(|loan| loan.is_valid())
        .collect();
    let suspending_loans: Vec<_> = live_loans
        .iter()
        .filter(|handle| is_exclusive(handle) && !lent_loans.contains(handle))
        .copied()
        .collect();
    if suspending_loans.is_empty() {
        return;
    }
    let receiver_reads = attached_places(
        program,
        machine,
        crate::flow::call_receiver_read_accesses(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            &facts.borrow,
            call,
            reads,
            None,
        ),
    );
    for loan_handle in suspending_loans {
        let loan = facts.borrow.loans.get(loan_handle);
        let loan_place = attached_place(
            program,
            machine,
            CapturedPlace {
                root_symbol: loan.root_symbol,
                segments: facts.borrow.loan_segments(loan).to_vec(),
            },
        );
        let Some(read) = receiver_reads.iter().find(|read| {
            !captured_place_compatibility(
                program,
                read,
                &BorrowAccessKind::Read,
                &loan_place,
                &loan.kind,
                stated_premises,
                bound_lookup,
            )
            .non_interfering
        }) else {
            continue;
        };
        diagnostics.push(Diagnostic::error(format!(
            "statement {} calls `{}`, whose mutable receiver reads `{}` while local borrow `{}` is still active ({})",
            call.statement_index,
            call_target_label(program, call.target_symbol),
            attached_label(program, state_flow, read),
            program.symbols.name(loan.owner_symbol),
            active_loan_detail(state_flow, facts, loan_handle, call.statement_index)
                .unwrap_or_else(|| format!("borrowed at statement {}", loan.statement_index)),
        )));
    }
}

/// Whether `call` is an authored receiver call, which returns to its caller,
/// on a target taking `&mut self`.
pub(in crate::checks::borrows) fn returns_through_mutable_receiver(
    program: &TypedTrees,
    call: &BorrowCallFact,
) -> bool {
    call.has_receiver && receiver_is_mutable_reference(program, call.target_symbol, false)
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
