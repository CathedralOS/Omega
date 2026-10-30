//! Join completed descriptor effects over an exact set of checked bodies.
//! Both target families and closed dynamic dispatch use this positional join.
//! An untouched slot retains its incoming alternative; unknown origins stay
//! unknown. The caller still owns invocation admission and argument capture.

use super::*;

pub(super) fn relative_effects(
    program: &dyn crate::CheckingInput,
    states: &[SymbolHandle],
    target_parameters: &[StateParameter],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let mut joined = Vec::<SlotEffect>::new();
    let mut processed = false;
    for &state_symbol in states {
        let state = program.state_by_symbol(state_symbol)?;
        let machine = program.machine_holding_state(state_symbol)?;
        let parameters = program.state_parameters(state);
        if parameters.len() != target_parameters.len()
            || !parameters
                .iter()
                .zip(target_parameters)
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
            effect.root = (remap_root(effect.root, parameters, target_parameters))?;
            if let Some(sources) = effect.sources.as_mut() {
                for source in sources {
                    (remap_place(&mut source.origin.source, parameters, target_parameters))?;
                    if !source.origin.source.is_static()
                        && source.origin.source.output_identity().is_none()
                    {
                        source.origin.path = super::path(program, &source.origin.source);
                    }
                    (remap_place(&mut source.carrier, parameters, target_parameters))?;
                    if let Some(backing) = source.carrier_backing.as_mut() {
                        (remap_place(&mut backing.source, parameters, target_parameters))?;
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
    target_parameters: &[StateParameter],
) -> Option<SymbolHandle> {
    let position = typed_trees::signature::state_parameter_position(parameters, root)?;
    Some(target_parameters.get(position)?.symbol)
}

fn remap_place(
    place: &mut FrameSourcePlace,
    parameters: &[StateParameter],
    target_parameters: &[StateParameter],
) -> Option<()> {
    if place.is_static() || place.output_identity().is_some() {
        return Some(());
    }
    place.root = remap_root(place.root, parameters, target_parameters)?;
    for recast in &mut place.recasts {
        recast.root = remap_root(recast.root, parameters, target_parameters)?;
    }
    Some(())
}
