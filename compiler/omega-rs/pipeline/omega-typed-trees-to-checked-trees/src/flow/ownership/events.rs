use super::*;

pub(in crate::flow::ownership) fn append_move_event_for_place(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    ctx.ownership.moves.append(FlowMoveEventFact {
        source,
        root: normalized_event_place_root(program, place.root),
        segments: ctx.ownership.segments.insert_many(place.segments),
    });
}

pub(in crate::flow::ownership) fn append_drop_event_for_place(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    ctx.ownership.drops.append(FlowDropEventFact {
        source,
        root: normalized_event_place_root(program, place.root),
        segments: ctx.ownership.segments.insert_many(place.segments),
    });
}

/// Re-root a `self`/`self.field` event place at its machine symbol.
///
/// A canonical place for `self.field` roots at the producing state's `&mut
/// self` parameter symbol, but every post-checked stage filters `self` out of
/// its parameter lists (the state graph drops it when scheduling parameters),
/// so a downstream consumer of the preserved ownership events could never
/// resolve that root again. The machine symbol is the durable identity of the
/// `self` instance, and it survives the whole spine.
fn normalized_event_place_root(
    program: &omega_typed_trees::TypedTrees,
    root: omega_facts::PlaceRoot,
) -> omega_facts::PlaceRoot {
    let omega_facts::PlaceRoot::Symbol(symbol) = root else {
        return root;
    };

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self && parameter.symbol == symbol)
            {
                return omega_facts::PlaceRoot::Symbol(machine.symbol);
            }
        }
    }

    root
}
