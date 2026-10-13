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
    effects
        .into_iter()
        .flat_map(|effect| {
            let owner = effect.destination.local_symbol;
            let owner_path =
                owner_path_from_place_segments(program, &effect.destination.local_segments);
            effect.sources.into_iter().map(move |source| {
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
                let mut matching = captured.iter().filter(|candidate| {
                    candidate.place.root == facts::PlaceRoot::Symbol(backing.root)
                        && candidate.place.segments == backing.segments
                });
                let supplying = match (matching.next(), matching.next()) {
                    (Some(supplying), None) => Some(supplying),
                    _ => None,
                };
                // Provenance alone cannot select a convenient same-backing
                // loan. Only the exact actual carrier supplies transfer
                // authority; otherwise retain the backing conflict without
                // inventing a parent.
                StatementBorrowLoan {
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
                        segments: backing.segments,
                    },
                    source_owner_symbol: supplying
                        .map_or(Default::default(), |source| source.source_owner_symbol),
                    supplying_loan: supplying
                        .map_or(Default::default(), |source| source.parent_loan),
                    lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: super::borrow_access_kind(effect.access),
                    call_result: true,
                }
            })
        })
        .collect()
}
