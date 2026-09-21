//! The declared-signature ceiling of a call whose write frame is unknown.
//!
//! An opaque, unresolved, generic or dispatched callee, or an actual whose
//! storage origin the frame resolver cannot summarize, retains "the
//! signature/authority ceiling", never an empty write set and never a guess
//! narrower than the declaration (wiki/spec/language/dependent_values.md,
//! "Mutation frames"). That ceiling is the storage reachable through the
//! call's exclusive borrows: each `&mut`/`&write` actual's exact storage and
//! the receiver of a `&mut self` callee. A by-value argument whose type
//! carries no exclusive reference hands the callee its own copy and reaches
//! no caller storage; one that may carry a reference, or an exclusive actual
//! with no representable storage origin, leaves the ceiling unrepresentable,
//! and the caller keeps retiring every live fact as before.

use crate::flow::CanonicalPlace;
use checked_trees::{BorrowCallFact, BorrowFacts, BorrowLoanOwnerSegment};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceNode;

/// The storage an unknown-frame call may write, with each exclusive actual's
/// own alias place beside its storage so facts keyed on the alias retire
/// too. The alias closure runs in the caller (`call_storage_writes`).
pub(crate) fn signature_ceiling_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    signature_ceiling_places_inner(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        None,
        borrow_call,
        call_frames,
    )
}

/// The same ceiling with the borrow ledger available: by-value actuals that
/// may carry references resolve to the exact referents of their carried
/// exclusive loans instead of leaving the ceiling unrepresentable.
pub(crate) fn signature_ceiling_places_with_loans(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    signature_ceiling_places_inner(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        Some(borrow),
        borrow_call,
        call_frames,
    )
}

fn signature_ceiling_places_inner(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: Option<&BorrowFacts>,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let machine = crate::lookup::machine_by_symbol(program, caller_machine_symbol)?;
    let state = crate::semantic_calls::find_state(program, caller_state_symbol)?;
    // A builtin function (`min`, `max`, `sqrt`, the float classifiers, the
    // asm intrinsics) takes scalar values and owns no caller storage; its
    // ceiling is empty even though it declares no state parameters.
    if program
        .symbols
        .builtin_function_for_symbol(borrow_call.target_symbol)
        .is_some()
    {
        return Some(Vec::new());
    }
    let site = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let parameters =
        crate::semantic_calls::call_target_parameters(program, borrow_call.target_symbol)?;
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &site);
    if parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count()
        != arguments.len()
    {
        return None;
    }
    let mut owned_frames = None;
    let resolver = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames);
    let mut places = Vec::new();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let actual = if parameter.is_self {
            if !is_exclusive_reference(program, parameter.type_reference) {
                continue;
            }
            crate::flow::canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &site,
                borrow_call.statement_index,
            )?
        } else {
            let argument = arguments[argument_index];
            argument_index += 1;
            if !is_exclusive_reference(program, parameter.type_reference) {
                // Declared reference-free storage cannot reach the caller. A
                // by-value actual that may carry references is still exact:
                // every borrow inside a value is a recorded loan, so the
                // caller storage a callee could write is the referent set of
                // the actual's carried exclusive loans. Shared `&` loans
                // grant the callee reads only and reach nothing writable.
                if resolver.is_none_or(|resolver| {
                    resolver.local_requires_write_origin(parameter.type_reference)
                }) {
                    let referents = borrow.and_then(|borrow| {
                        carried_exclusive_referents(
                            program,
                            borrow,
                            state.symbol,
                            borrow_call.statement_index,
                            arguments[argument_index - 1],
                        )
                    })?;
                    for referent in referents {
                        if !places.contains(&referent) {
                            places.push(referent);
                        }
                    }
                }
                continue;
            }
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                borrow_call.statement_index,
                argument,
            )?
        };
        for storage in super::local_origins::rebase_local_write_places(
            program,
            state.symbol,
            borrow_call.statement_index,
            actual.clone(),
            call_frames,
        )? {
            if !places.contains(&storage) {
                places.push(storage);
            }
        }
        if !places.contains(&actual) {
            places.push(actual);
        }
    }
    Some(places)
}

pub(super) fn is_exclusive_reference(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let mut reference = type_reference;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => return access.is_exclusive(),
            _ => return false,
        }
    }
    false
}

/// The caller storage a consumed by-value actual hands the callee through
/// carried exclusive loans. `None` when the actual's storage cannot be named
/// exactly or a carried exclusive loan has no spellable referent — both keep
/// the whole ceiling unrepresentable rather than guessing narrower than the
/// declaration. Shared (`&`) loans contribute nothing: a read-only borrow
/// inside the consumed value reaches no writable caller place.
fn carried_exclusive_referents(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<CanonicalPlace>> {
    let actual = crate::flow::canonical_place_from_expression_in_state(
        program,
        caller_state_symbol,
        statement_index,
        argument,
    )?;
    let PlaceRoot::Symbol(owner_root) = actual.root else {
        return None;
    };
    let (_, caller_state) = borrow
        .states
        .iter()
        .find(|(_, state)| state.state_symbol == caller_state_symbol)?;
    let mut referents = Vec::new();
    for (handle, loan) in borrow.loans.iter() {
        if !borrow.state_owns_loan(caller_state, handle)
            || !loan.kind.is_exclusive()
            || loan.owner_symbol != owner_root
            || !owner_path_overlaps(program, borrow.loan_owner_path(loan), &actual.segments)
        {
            continue;
        }
        // An exclusive loan whose referent root cannot be named leaves the
        // ceiling unrepresentable: the caller cannot bound what it writes.
        if !loan.root_symbol.is_valid() {
            return None;
        }
        let referent = CanonicalPlace {
            root: PlaceRoot::Symbol(loan.root_symbol),
            segments: borrow.loan_segments(loan).to_vec(),
        };
        if !referents.contains(&referent) {
            referents.push(referent);
        }
    }
    Some(referents)
}

/// Whether a recorded loan's owner path and a place's field path overlap.
/// Same prefix law as `borrow::last_uses`'s owner-path matching: an absent
/// or dynamic selector conservatively overlaps its counterpart.
fn owner_path_overlaps(
    program: &typed_trees::TypedTrees,
    owner_path: &[BorrowLoanOwnerSegment],
    place_segments: &[facts::PlaceSegment],
) -> bool {
    owner_path
        .iter()
        .zip(place_segments)
        .all(|(owner, place)| match (owner, place) {
            (
                BorrowLoanOwnerSegment::Field(owner_symbol),
                facts::PlaceSegment::Field {
                    symbol: place_symbol,
                },
            ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
            (
                BorrowLoanOwnerSegment::Case(owner_variant),
                facts::PlaceSegment::Case {
                    variant: place_variant,
                },
            ) => owner_variant == place_variant,
            (
                BorrowLoanOwnerSegment::FixedIndex(owner_index),
                facts::PlaceSegment::FixedIndex { index: place_index },
            ) => owner_index == place_index,
            (
                BorrowLoanOwnerSegment::FixedIndex(owner_index),
                facts::PlaceSegment::Index { expression },
            ) => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .is_none_or(|place_index| *owner_index == place_index),
            (
                BorrowLoanOwnerSegment::DynamicIndex,
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
            ) => true,
            _ => false,
        })
}
