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
    let mut joined = Vec::<SlotEffect>::new();
    let mut processed = false;
    for state_symbol in states {
        let state = program.state_by_symbol(state_symbol)?;
        let machine = program.machine_holding_state(state_symbol)?;
        let parameters = program.state_parameters(state);
        if parameters.len() != slot.data_parameters().len()
            || !parameters
                .iter()
                .zip(slot.data_parameters())
                .all(|(source, target)| {
                    crate::storage_origin_types_match(
                        program,
                        source.type_reference,
                        target.type_reference,
                    )
                })
        {
            return None;
        }
        let mut effects = (super::state_effects(program, machine, state, symbols, inference))?;
        for effect in &mut effects {
            effect.root = (remap_root(effect.root, parameters, slot))?;
            if let Some(sources) = effect.sources.as_mut() {
                for source in sources {
                    (remap_place(&mut source.origin.source, parameters, slot))?;
                    if !source.origin.source.is_static()
                        && source.origin.source.output_identity().is_none()
                    {
                        source.origin.path = super::path(program, &source.origin.source);
                    }
                    (remap_place(&mut source.carrier, parameters, slot))?;
                    if let Some(backing) = source.carrier_backing.as_mut() {
                        (remap_place(&mut backing.source, parameters, slot))?;
                        if !backing.source.is_static() && backing.source.output_identity().is_none()
                        {
                            backing.path = super::path(program, &backing.source);
                        }
                    }
                }
            }
        }
        // A complete realization that leaves a joined slot untouched retains
        // its incoming descriptor. Missing a write is not a missing route.
        for prior in &mut joined {
            if !effects.iter().any(|effect| same_slot(prior, effect)) {
                prior.incoming = true;
            }
        }
        for mut effect in effects {
            if let Some(prior) = joined.iter_mut().find(|prior| same_slot(prior, &effect)) {
                prior.incoming |= effect.incoming;
                prior.may_select |= effect.may_select;
                match (&mut prior.sources, effect.sources) {
                    (Some(prior), Some(sources)) => {
                        for source in sources {
                            if !prior.contains(&source) {
                                prior.push(source);
                            }
                        }
                    }
                    (target, _) => *target = None,
                }
            } else {
                effect.incoming |= processed;
                joined.push(effect);
            }
        }
        processed = true;
    }
    Some(joined)
}

fn same_slot(left: &SlotEffect, right: &SlotEffect) -> bool {
    left.root == right.root && left.segments == right.segments && left.kind == right.kind
}

fn remap_root(
    root: SymbolHandle,
    parameters: &[StateParameter],
    slot: &DynamicSlotSignature,
) -> Option<SymbolHandle> {
    let position = typed_trees::signature::state_parameter_position(parameters, root)?;
    Some(slot.data_parameters().get(position)?.symbol)
}

fn remap_place(
    place: &mut FrameSourcePlace,
    parameters: &[StateParameter],
    slot: &DynamicSlotSignature,
) -> Option<()> {
    if place.is_static() || place.output_identity().is_some() {
        return Some(());
    }
    place.root = remap_root(place.root, parameters, slot)?;
    for recast in &mut place.recasts {
        recast.root = remap_root(recast.root, parameters, slot)?;
    }
    Some(())
}
