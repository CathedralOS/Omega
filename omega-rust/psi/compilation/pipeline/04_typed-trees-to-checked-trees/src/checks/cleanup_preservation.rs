//! Preserve original observations across actual selected cleanup writes.
//!
//! Permission compatibility is not spatial disjointness. A reference descriptor
//! captures a storage identity, not the contents read through it. This query
//! consumes the original exit's complete caller-relative hook footprints; it
//! grants neither a new predicate nor a loan and does not replay any operand.
use checked_trees::{CheckFacts, FlowClaimJoinExit};
use validation::CanonicalPlace;

pub(super) fn reads_survive(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    exit: FlowClaimJoinExit,
    reads: &[CanonicalPlace],
) -> bool {
    facts
        .contextual_cleanup
        .hook_effects
        .iter()
        .all(|(_, effect)| {
            let disposition = facts
                .contextual_cleanup
                .dispositions
                .get(effect.disposition);
            if disposition.machine != machine || disposition.exit != exit {
                return true;
            }
            // An authenticated captured value with no live storage observations
            // cannot be changed by a hook's writes, including opaque writes.
            if reads.is_empty() {
                return true;
            }
            if !effect.complete {
                return false;
            }
            let Some(writes) = facts
                .contextual_cleanup
                .hook_write_places
                .span(effect.writes)
            else {
                return false;
            };
            reads.iter().all(|read| {
                writes.iter().all(|write| {
                    let Some(segments) = facts
                        .contextual_cleanup
                        .hook_write_segments
                        .span(write.segments)
                    else {
                        return false;
                    };
                    let write = CanonicalPlace {
                        root: write.root,
                        segments: segments.to_vec(),
                    };
                    places_are_disjoint(program, exit, read, &write)
                })
            })
        })
}

fn places_are_disjoint(
    program: &dyn validation::CheckingInput,
    exit: FlowClaimJoinExit,
    read: &CanonicalPlace,
    write: &CanonicalPlace,
) -> bool {
    if !read.root.is_valid() || !write.root.is_valid() {
        return false;
    }
    if read.root == write.root {
        if crate::flow::canonical_place_segments_may_overlap(
            program,
            &read.segments,
            &write.segments,
        ) {
            return false;
        }
        // Divergent fields can contain descriptors into the same bytes too.
        // A lexical divergence before a reference crossing is not spatial
        // separation of the observations beyond that crossing.
        return owns_observed_storage(program, exit, read)
            && owns_observed_storage(program, exit, write);
    }
    // Local-origin alias expansion does not prove incoming descriptors denote
    // distinct bytes. Only an owned storage path with no reference crossing
    // can establish separation from another independently owned root here.
    owns_observed_storage(program, exit, read) && owns_observed_storage(program, exit, write)
}

fn owns_observed_storage(
    program: &dyn validation::CheckingInput,
    exit: FlowClaimJoinExit,
    place: &CanonicalPlace,
) -> bool {
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    let Some(reference) = validation::canonical_place_type_reference(
        program,
        exit.state_symbol,
        exit.statement_index,
        place,
    ) else {
        return false;
    };
    // The owner-path query describes the descriptor's own storage at a final
    // reference leaf. That does not prove its observed referent is owned here.
    if validation::reference_shell(program, reference).is_some() {
        return false;
    }
    // Complete borrowed-descriptor-free contents are an existing owned
    // storage judgment, including erased fields and generic substitutions.
    // A nonzero unresolved type handle cannot establish that separation.
    if !validation::has_contents(
        program,
        reference,
        &[],
        validation::ContentsRequirement::BorrowFree,
    ) {
        return false;
    }
    let mut path = Vec::with_capacity(place.segments.len());
    for segment in &place.segments {
        path.push(match segment {
            facts::PlaceSegment::Field { symbol } => {
                checked_trees::BorrowLoanOwnerSegment::Field(*symbol)
            }
            facts::PlaceSegment::Case { variant } => {
                checked_trees::BorrowLoanOwnerSegment::Case(*variant)
            }
            facts::PlaceSegment::FixedIndex { index } => {
                checked_trees::BorrowLoanOwnerSegment::FixedIndex(*index)
            }
            facts::PlaceSegment::Index { .. } => {
                checked_trees::BorrowLoanOwnerSegment::DynamicIndex
            }
            // A collection window needs its own spatial proof; interpreting
            // its root symbols as disjoint would invent that proof.
            facts::PlaceSegment::FixedRange { .. } => return false,
        });
    }
    validation::loan_carrier_has_owned_storage(
        program,
        exit.state_symbol,
        exit.statement_index,
        root,
        &path,
    )
}
