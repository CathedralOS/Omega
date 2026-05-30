use super::*;

pub(in crate::flow::ownership) fn append_move_event_for_place(
    ctx: &mut FlowBuildContext,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    ctx.ownership.moves.append(FlowMoveEventFact {
        source,
        root: place.root,
        segments: ctx.ownership.segments.insert_many(place.segments),
    });
}

pub(in crate::flow::ownership) fn append_drop_event_for_place(
    ctx: &mut FlowBuildContext,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    ctx.ownership.drops.append(FlowDropEventFact {
        source,
        root: place.root,
        segments: ctx.ownership.segments.insert_many(place.segments),
    });
}
