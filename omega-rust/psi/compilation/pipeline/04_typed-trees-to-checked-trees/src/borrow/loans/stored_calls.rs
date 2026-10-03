//! A Unit call can install a reference in a caller-owned field even though it
//! returns no value. Its body-derived slot relation supplies the same exact
//! owner/backing pairs as an assignment; type shape alone supplies no loan.

use super::{StatementBorrowLoan, owner_path_from_place_segments};

pub(super) fn loans(
    program: &typed_trees::TypedTrees,
    _state: &typed_trees::state::State,
    machine_symbol: symbols::SymbolHandle,
    statement: &checked_trees::statement::StatementNode,
) -> Vec<StatementBorrowLoan> {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return Vec::new();
    };
    let Some(effects) = validation::CallFrameResolver::new(program)
        .and_then(|frames| frames.stored_reference_effects(machine, statement))
    else {
        return Vec::new();
    };
    effects
        .into_iter()
        .flat_map(|effect| {
            let owner = effect.destination.local_symbol;
            let owner_path =
                owner_path_from_place_segments(program, &effect.destination.local_segments);
            effect
                .sources
                .into_iter()
                .map(move |source| StatementBorrowLoan {
                    owner_symbol: owner,
                    owner_name: program.symbols.name(owner).into(),
                    owner_path: owner_path.clone(),
                    place: checked_trees::CapturedPlace {
                        root_symbol: source.root,
                        segments: source.segments,
                    },
                    source_owner_symbol: source.root,
                    lineage: checked_trees::BorrowLoanLineage::UnretainedDerived,
                    kind: checked_trees::BorrowAccessKind::Read,
                    call_result: true,
                })
        })
        .collect()
}
