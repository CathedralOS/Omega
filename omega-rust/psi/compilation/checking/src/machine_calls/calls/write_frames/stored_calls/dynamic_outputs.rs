//! Dispatch output custody is the union of every closed realization's summary.
//!
//! This judgment consumes the same authenticated selector roster as dispatch
//! and termination; it never chooses a representative body. Formal coordinates
//! map by position after the selector lane. Unknown bodies/sources withhold
//! coverage, and opaque lifetime dependencies remain nonspatial obligations.
//! No predicate or private postcondition is imported through this relation.

use super::*;
use typed_trees::dynamic_traits::DynamicSlotSignature;

pub(in crate::machine_calls::calls::write_frames) fn relative_effects(
    program: &dyn crate::CheckingInput,
    slot: &DynamicSlotSignature,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let selections = (crate::collect_dynamic_implementation_selections(program).ok())?;
    let mut states = Vec::new();
    for selection in selections.iter().filter(|selection| {
        program.normalized_type_identity(selection.selector_type) == slot.interface_identity
    }) {
        let conformance = (program.conformance_by_symbol(selection.implementation))?;
        let rows = conformance
            .rows
            .iter()
            .filter(|row| {
                row.requirement == slot.requirement && row.declaring_trait == slot.declaring_trait
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return None;
        }
        for row in rows {
            let machine = (program.machine_by_symbol(row.realization_machine))?;
            let state = (program.state_in_machine(machine, row.realization_state))?;
            if !machine.body_is_present
                || !machine.supply_mode.is_checked_body()
                || !program.machine_type_parameters(machine).is_empty()
                || program.machine_states(machine).first()?.symbol != state.symbol
            {
                return None;
            }
            if !states.contains(&state.symbol) {
                states.push(state.symbol);
            }
        }
    }
    if states.is_empty() {
        return None;
    }
    super::checked_alternatives::relative_effects(
        program,
        &states,
        slot.data_parameters(),
        symbols,
        inference,
    )
}
