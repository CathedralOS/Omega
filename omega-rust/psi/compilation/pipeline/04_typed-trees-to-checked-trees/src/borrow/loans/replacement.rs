//! Capture a store's descriptor subtree before evaluating its value. Both
//! current-loan retirement and final receiving-loan formation use this same
//! destination; RHS effects cannot redirect it or survive its replacement.

use super::super::tracker::{BorrowOwnerSegment, StateLoanTracker};

pub(in crate::borrow) struct CarrierReplacement {
    destination: checked_trees::CapturedPlace,
    preserves_root_reference: bool,
}

impl CarrierReplacement {
    pub(in crate::borrow) fn replaces(
        &self,
        program: &dyn validation::CheckingInput,
        owner: facts::PlaceRoot,
        path: &[BorrowOwnerSegment],
    ) -> bool {
        owner == self.destination.root
            && !(self.preserves_root_reference && path.len() == self.destination.segments.len())
            && super::place_path_covers_owner(program, &self.destination.segments, path)
    }
}

pub(in crate::borrow) fn capture_replacement(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &typed_trees::statement::StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    current: &[StateLoanTracker],
) -> Option<CarrierReplacement> {
    let assignment = statement.as_assignment()?;
    let writes_referent =
        if validation::declared_place_type_raw(program, machine, Some(state), assignment.target)
            .is_some_and(|reference| validation::reference_shell(program, reference).is_some())
        {
            !call_frames?.assignment_replaces_reference_binding(machine, statement)?
        } else {
            false
        };
    let target = super::borrow_access_place(
        program,
        state.symbol,
        statement_index,
        assignment.target,
        machine.symbol,
    )?;
    let destinations = super::store_destination_places(
        program,
        state,
        target.clone(),
        writes_referent,
        current.iter().map(|loan| {
            (
                loan.storage_owner,
                loan.owner_path.as_slice(),
                loan.place.incoming_reference,
                loan.place.root,
                loan.place.segments.as_slice(),
            )
        }),
    );
    let [destination] = destinations.as_slice() else {
        // A may-alias target cannot identify a definitely replaced slot.
        return None;
    };
    Some(CarrierReplacement {
        destination: destination.clone(),
        preserves_root_reference: writes_referent && destination == &target,
    })
}
