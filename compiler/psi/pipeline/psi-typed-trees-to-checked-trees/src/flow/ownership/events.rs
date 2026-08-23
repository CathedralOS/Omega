use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowOwnershipEventSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiscoveredMoveEvent {
    pub(crate) source: FlowOwnershipEventSource,
    pub(crate) root: psi_facts::PlaceRoot,
    pub(crate) segments: HandleSpan<psi_facts::PlaceSegment>,
}

pub(crate) struct DirectMoveEventSink<'segments> {
    segments: &'segments mut psi_arena::Arena<psi_facts::PlaceSegment>,
    events: Vec<DiscoveredMoveEvent>,
}

impl<'segments> DirectMoveEventSink<'segments> {
    pub(crate) fn new(segments: &'segments mut psi_arena::Arena<psi_facts::PlaceSegment>) -> Self {
        Self {
            segments,
            events: Vec::new(),
        }
    }

    pub(crate) fn finish(self) -> Vec<DiscoveredMoveEvent> {
        self.events
    }
}

impl DirectMoveEventSink<'_> {
    fn append_move_event(
        &mut self,
        program: &psi_typed_trees::TypedTrees,
        place: CanonicalPlace,
        source: FlowOwnershipEventSource,
    ) {
        self.events.push(DiscoveredMoveEvent {
            source,
            root: normalized_event_place_root(program, place.root),
            segments: self.segments.insert_many(place.segments),
        });
    }
}

pub(in crate::flow::ownership) fn append_move_event_for_place(
    program: &psi_typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
) {
    sink.append_move_event(program, place, source);
}

/// Re-root a `self`/`self.field` event place at its machine symbol.
///
/// A canonical place for `self.field` roots at the producing state's `&mut
/// self` parameter symbol, but every post-checked stage filters `self` out of
/// its parameter lists (the state graph drops it when scheduling parameters),
/// so the permission producer could not publish a durable root for it. The
/// machine symbol is the durable identity of the `self` instance.
pub(crate) fn normalized_event_place_root(
    program: &psi_typed_trees::TypedTrees,
    root: psi_facts::PlaceRoot,
) -> psi_facts::PlaceRoot {
    let psi_facts::PlaceRoot::Symbol(symbol) = root else {
        return root;
    };

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self && parameter.symbol == symbol)
            {
                return psi_facts::PlaceRoot::Symbol(machine.symbol);
            }
        }
    }

    root
}
