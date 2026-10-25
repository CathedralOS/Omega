//! A completed descriptor load crosses its typed receivers, then its own slot.
//!
//! The requested leaf is not the address of its enclosing pointer's referent.
//! Keep the unconsumed typed suffix separate from each captured backing address:
//! `&Record` followed by a reference field owes two loads, while `&&Value` with
//! no suffix owes just the outer load. Every step uses the original occurrence's
//! complete constraint cut. No resulting address is repeatedly loaded merely
//! because storage at that address can itself contain a reference.
//!
//! These vectors are one query's construction scratch. Existing loan and ingress
//! handles retain all authority; no new durable proof, pointer or lifetime is
//! manufactured by this traversal.

use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowLoanFact, BorrowLoanOwnerSegment, CapturedPlace, CheckFacts, FlowConstraintRef,
    FlowStateFact, IncomingReferenceIdentity,
};
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

use super::super::aliases::{self, ResolvedAlias};

/// A resource occurrence plus its remaining typed request. Repeating a graph
/// edge after consuming a path segment is finite and legitimate; repeating the
/// same resource and request cannot finish and supplies no authority.
#[derive(Clone, PartialEq, Eq)]
struct LoadOccurrence {
    loan: Handle<BorrowLoanFact>,
    incoming: Handle<IncomingReferenceIdentity>,
    descriptor_type: TypeReferenceHandle,
    remaining: Vec<PlaceSegment>,
}

struct PendingLoad {
    /// Address in this namespace before traversing `remaining`.
    address: CapturedPlace,
    /// Declared presentation at that address, after the preceding load.
    carrier: TypeReferenceHandle,
    remaining: Vec<PlaceSegment>,
    ancestors: Vec<LoadOccurrence>,
    /// A borrowed formal supplies its outer grant separately from the stored
    /// ingress leaves of its referee. In `&&T`, both have the empty owner path.
    formal_root: bool,
}

pub(super) fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    constraints: HandleSpan<FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedAlias>> {
    match resolve_loads(
        program,
        facts,
        state,
        statement_index,
        constraints,
        place.clone(),
    ) {
        Ok(resolved) => Some(resolved),
        Err(reason) => {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "CHECKSITE descriptor-load state={:?} statement={} constraints={:?} place={:?} refusal={}",
                    state.state_symbol, statement_index, constraints, place, reason,
                );
            }
            None
        }
    }
}

fn resolve_loads(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement_index: usize,
    constraints: HandleSpan<FlowConstraintRef>,
    place: CapturedPlace,
) -> Result<Vec<ResolvedAlias>, &'static str> {
    let mut address = place.clone();
    address.segments.clear();
    let carrier = if place.incoming_reference.is_valid() {
        let incoming = current_incoming(facts, state, constraints, &place)
            .ok_or("the source has no exact current incoming namespace")?;
        let owner = facts
            .borrow
            .owner_segments
            .span(incoming.owner_path)
            .ok_or("the incoming source has no complete owner path")?;
        let descriptor = validation::owned_projection_type_reference(
            program,
            incoming.carrier_type,
            &crate::borrow::owner_place_segments(owner),
        )
        .ok_or("the incoming source has no declared descriptor type")?;
        validation::reference_shell(program, descriptor)
            .ok_or("the incoming source carries a region rather than a pointer")?
            .referee
    } else {
        validation::canonical_place_type_reference(
            program,
            state.state_symbol,
            statement_index,
            &CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )
        .ok_or("the descriptor root has no declared carrier type")?
    };
    let formal_root = !place.incoming_reference.is_valid()
        && program
            .state_by_symbol(state.state_symbol)
            .is_some_and(|declaration| {
                program
                    .state_parameters(declaration)
                    .iter()
                    .any(|parameter| {
                        place.root == facts::PlaceRoot::Symbol(parameter.symbol)
                            && !parameter.is_const
                            && !parameter.relevance.is_erased()
                            && validation::reference_shell(program, parameter.type_reference)
                                .is_some()
                    })
            });
    let mut pending = vec![PendingLoad {
        address,
        carrier,
        remaining: place.segments,
        ancestors: Vec::new(),
        formal_root,
    }];
    let mut resolved = Vec::new();
    while let Some(cursor) = pending.pop() {
        // Only owned prefixes can precede the next reference. This common
        // projector retains the supplying generic application at every prefix.
        let (prefix, descriptor_type) =
            first_descriptor(program, cursor.carrier, &cursor.remaining)
                .ok_or("the remaining typed suffix has no finite reference descriptor")?;
        let reference = validation::reference_shell(program, descriptor_type)
            .ok_or("the selected typed boundary is not a reference")?;
        let terminal = prefix == cursor.remaining.len();
        // Loading the final pointer does not read its pointee. Reaching another
        // descriptor through that pointer does, including an empty next prefix.
        if !terminal && !reference.access.is_readable() {
            return Err("the descriptor receiver is write-only");
        }
        let mut descriptor = cursor.address.clone();
        descriptor
            .segments
            .extend_from_slice(&cursor.remaining[..prefix]);
        let remaining = &cursor.remaining[prefix..];

        if cursor.formal_root && prefix == 0 {
            // This is the declaration's outer borrowed grant, never a lookup
            // of an interior ingress row with a coincident empty owner path.
            // Ordinary use/move/initialization checking still owns formation.
            if terminal {
                append_distinct(
                    &mut resolved,
                    ResolvedAlias {
                        place: descriptor,
                        lineage: Vec::new(),
                    },
                );
            } else {
                pending.push(PendingLoad {
                    address: descriptor,
                    carrier: reference.referee,
                    remaining: remaining.to_vec(),
                    ancestors: cursor.ancestors,
                    formal_root: false,
                });
            }
            continue;
        }

        let lookup = descriptor_lookup(facts, state, constraints, &descriptor)
            .ok_or("the selected slot has no exact current descriptor namespace")?;
        let candidates =
            aliases::resolve_descriptor_places(program, facts, state, constraints, lookup.clone())
                .ok_or("the selected slot has no complete current occurrence roster")?;
        if candidates.is_empty() {
            return Err("the selected descriptor has no authority alternatives");
        }
        // The typed walk already consumed every receiver crossing. A shallower
        // owner is not the selected slot and cannot substitute its own pointer.
        let mut occurrences = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            let (owner, incoming) = if candidate.local_loan.is_valid() {
                let loan = facts.borrow.loans.get(candidate.local_loan);
                if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                    return Err("a lifetime dependency cannot supply a descriptor address");
                }
                (
                    facts.borrow.loan_owner_path(loan),
                    candidate.place.incoming_reference,
                )
            } else {
                let incoming = current_incoming(facts, state, constraints, &candidate.place)
                    .ok_or("an authority alternative has no exact current incoming identity")?;
                (
                    facts
                        .borrow
                        .owner_segments
                        .span(incoming.owner_path)
                        .ok_or("an incoming alternative has an incomplete owner path")?,
                    candidate.place.incoming_reference,
                )
            };
            if owner.len() != lookup.segments.len() {
                return Err("an enclosing descriptor cannot supply the selected slot's pointer");
            }
            occurrences.push(LoadOccurrence {
                loan: candidate.local_loan,
                incoming,
                descriptor_type,
                remaining: remaining.to_vec(),
            });
        }
        let alternatives = super::retain_captured_lineage(
            program,
            facts,
            state,
            statement_index,
            lookup,
            candidates,
        )
        .ok_or("a selected descriptor alternative has incomplete captured lineage")?;
        if alternatives.len() != occurrences.len() {
            return Err("the selected occurrence roster lost an authority alternative");
        }
        for (alternative, occurrence) in alternatives.into_iter().zip(occurrences) {
            if cursor.ancestors.contains(&occurrence) {
                return Err("the typed descriptor request repeats a captured occurrence");
            }
            // A receiver authenticates reading this descriptor slot. It does
            // not become an access ancestor of the independently stored
            // pointer's pointee. Preserve this selected occurrence's exact
            // supplier lineage; lifetime debts remain a separate loan cohort.
            if terminal {
                append_distinct(&mut resolved, alternative);
            } else {
                let mut ancestors = cursor.ancestors.clone();
                ancestors.push(occurrence);
                pending.push(PendingLoad {
                    address: alternative.place,
                    carrier: reference.referee,
                    remaining: remaining.to_vec(),
                    ancestors,
                    formal_root: false,
                });
            }
        }
    }
    if resolved.is_empty() {
        Err("the completed descriptor has no resolved authority")
    } else {
        Ok(resolved)
    }
}

/// Find the next physical descriptor without implicitly crossing it. The final
/// reference is selected even when its referee is another reference.
fn first_descriptor(
    program: &dyn validation::CheckingInput,
    carrier: TypeReferenceHandle,
    remaining: &[PlaceSegment],
) -> Option<(usize, TypeReferenceHandle)> {
    for prefix in 0..=remaining.len() {
        let reference =
            validation::owned_projection_type_reference(program, carrier, &remaining[..prefix])?;
        if validation::reference_shell(program, reference).is_some() {
            return Some((prefix, reference));
        }
    }
    None
}

/// An opaque referent stays in the exact namespace that supplied it. Reattach
/// only that identity's declared carrier path for the existing occurrence
/// lookup; the returned authority retains the original opaque identity.
fn descriptor_lookup(
    facts: &CheckFacts,
    state: &FlowStateFact,
    constraints: HandleSpan<FlowConstraintRef>,
    place: &CapturedPlace,
) -> Option<CapturedPlace> {
    if !place.incoming_reference.is_valid() {
        return Some(place.clone());
    }
    let incoming = current_incoming(facts, state, constraints, place)?;
    let owner = facts.borrow.owner_segments.span(incoming.owner_path)?;
    if owner.contains(&BorrowLoanOwnerSegment::DynamicIndex) {
        return None;
    }
    let mut segments = crate::borrow::owner_place_segments(owner);
    segments.extend_from_slice(&place.segments);
    Some(CapturedPlace {
        incoming_reference: Handle::invalid(),
        root: incoming.place_root(),
        segments,
    })
}

fn current_incoming<'facts>(
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    constraints: HandleSpan<FlowConstraintRef>,
    place: &CapturedPlace,
) -> Option<&'facts IncomingReferenceIdentity> {
    let identity = place.incoming_reference;
    if !facts.borrow.incoming_references.is_valid(identity)
        || !facts
            .flow
            .incoming_reference_constraints(constraints)
            .any(|candidate| candidate == identity)
    {
        return None;
    }
    let (_, borrow_state) = facts
        .borrow
        .first_state_borrow(state.machine_symbol, state.state_symbol)?;
    if !facts
        .borrow
        .state_owns_incoming_reference(borrow_state, identity)
    {
        return None;
    }
    let incoming = facts.borrow.incoming_references.get(identity);
    (incoming.machine_symbol == state.machine_symbol
        && incoming.state_symbol == state.state_symbol
        && incoming.place_root() == place.root)
        .then_some(incoming)
}

fn append_distinct(resolved: &mut Vec<ResolvedAlias>, alternative: ResolvedAlias) {
    if !resolved.iter().any(|previous| {
        previous.place == alternative.place && previous.lineage == alternative.lineage
    }) {
        resolved.push(alternative);
    }
}
