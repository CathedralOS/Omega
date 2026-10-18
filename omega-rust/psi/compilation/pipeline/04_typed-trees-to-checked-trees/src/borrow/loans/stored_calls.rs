//! A call can install a reference in caller-owned storage independently of its
//! result value. Its body-derived slot relation supplies the same exact
//! owner/backing pairs as an assignment; type shape alone supplies no loan.

use super::{StatementBorrowLoan, owner_path_from_place_segments};

pub(super) fn loans(
    program: &dyn ::validation::CheckingInput,
    _state: &typed_trees::state::State,
    machine_symbol: symbols::SymbolHandle,
    statement: &checked_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    loan_trackers: &[super::StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(machine) = program.machine(machine_symbol) else {
        return Vec::new();
    };
    let Some(effects) =
        call_frames.and_then(|frames| frames.stored_reference_effects(machine, statement))
    else {
        return Vec::new();
    };
    let mut loans = Vec::new();
    for effect in effects {
        let owner = effect.destination.local_symbol;
        let owner_path =
            owner_path_from_place_segments(program, &effect.destination.local_segments);
        // An exact slot has one post-call occurrence. A route which does
        // not write contributes its incoming alternatives to that same
        // occurrence, not another simultaneously live descriptor. Capture
        // these transfers before the caller retires the old slot. Weak
        // stores keep their physical untouched slots instead.
        if effect.retains_incoming && !effect.may_select {
            for source in loan_trackers.iter().filter(|source| {
                source.storage_owner == facts::PlaceRoot::Symbol(owner)
                    && super::place_path_covers_owner(
                        program,
                        &effect.destination.local_segments,
                        &source.owner_path,
                    )
            }) {
                loans.push(StatementBorrowLoan {
                    incoming_origin: None,
                    formation_expression: Default::default(),
                    owner_symbol: owner,
                    storage_owner: facts::PlaceRoot::Symbol(owner),
                    owner_name: program.symbols.name(owner).into(),
                    owner_path: source.owner_path.clone(),
                    retained_operand: source.retained_operand,
                    place: source.place.clone(),
                    source_owner_symbol: source.owner_symbol,
                    // Preserve ingress identity in the captured place without
                    // manufacturing a local predecessor from its zero handle.
                    supplying_loans: std::iter::once(source.handle)
                        .filter(|handle| handle.is_valid())
                        .collect(),
                    lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: source.kind.clone(),
                    call_result: true,
                });
            }
        }
        for source in effect.sources {
            let backing = source.backing;
            let captured = super::call_views::rebase_borrow_places_through_local_loans(
                program,
                checked_trees::CapturedPlace {
                    incoming_reference: Default::default(),
                    root: facts::PlaceRoot::Symbol(source.carrier_symbol),
                    segments: source.carrier_segments,
                },
                loan_trackers,
            );
            let matching: Vec<_> = captured
                .iter()
                .filter(|candidate| {
                    candidate.place.root == facts::PlaceRoot::Symbol(backing.root)
                        && candidate.place.segments == backing.segments
                })
                .map(Some)
                .collect();
            // Equal geometry can belong to several alternatives of the
            // actual descriptor. Preserve every supplier, never choose one
            // by position or erase their identities by deduplicating places.
            let matching = if matching.is_empty() {
                vec![None]
            } else {
                matching
            };
            for supplying in matching {
                // Provenance alone cannot select a convenient same-backing
                // loan. Only the exact actual carrier supplies transfer
                // authority; otherwise retain the backing conflict without
                // inventing a parent.
                loans.push(StatementBorrowLoan {
                    incoming_origin: None,
                    formation_expression: Default::default(),
                    owner_symbol: owner,
                    storage_owner: facts::PlaceRoot::Symbol(owner),
                    owner_name: program.symbols.name(owner).into(),
                    owner_path: owner_path.clone(),
                    retained_operand: supplying.and_then(|source| source.retained_operand),
                    place: checked_trees::CapturedPlace {
                        incoming_reference: supplying
                            .map_or(Default::default(), |source| source.place.incoming_reference),
                        root: facts::PlaceRoot::Symbol(backing.root),
                        segments: backing.segments.clone(),
                    },
                    source_owner_symbol: supplying
                        .map_or(Default::default(), |source| source.source_owner_symbol),
                    supplying_loans: supplying
                        .map(|source| source.parent_loan)
                        .filter(|loan| loan.is_valid())
                        .into_iter()
                        .collect(),
                    lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: super::borrow_access_kind(effect.access),
                    call_result: true,
                });
            }
        }
    }
    loans
}
