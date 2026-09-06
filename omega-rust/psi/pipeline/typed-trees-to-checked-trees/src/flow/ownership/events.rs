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
    pub(crate) root: facts::PlaceRoot,
    pub(crate) segments: HandleSpan<facts::PlaceSegment>,
}

pub(crate) struct DirectMoveEventSink<'segments> {
    segments: &'segments mut arena::Arena<facts::PlaceSegment>,
    events: Vec<DiscoveredMoveEvent>,
    proof_only: Option<typed_trees::proof_only::ProofOnlyClassification>,
}

impl<'segments> DirectMoveEventSink<'segments> {
    pub(crate) fn new(segments: &'segments mut arena::Arena<facts::PlaceSegment>) -> Self {
        Self {
            segments,
            events: Vec::new(),
            proof_only: None,
        }
    }

    pub(crate) fn finish(self) -> Vec<DiscoveredMoveEvent> {
        self.events
    }
}

impl DirectMoveEventSink<'_> {
    pub(super) fn proof_only(
        &mut self,
        program: &typed_trees::TypedTrees,
    ) -> &typed_trees::proof_only::ProofOnlyClassification {
        self.proof_only
            .get_or_insert_with(|| typed_trees::proof_only::classify(program))
    }

    fn append_move_event(
        &mut self,
        program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    root: facts::PlaceRoot,
) -> facts::PlaceRoot {
    let facts::PlaceRoot::Symbol(symbol) = root else {
        return root;
    };

    let metadata = program.symbols.get(symbol);
    if metadata.kind != symbols::SymbolKind::Parameter {
        return root;
    }
    let parent = program.symbols.get(metadata.parent);
    let (machine_symbol, state_symbol) = match parent.kind {
        symbols::SymbolKind::Machine => (metadata.parent, SymbolHandle::invalid()),
        symbols::SymbolKind::State => (parent.parent, metadata.parent),
        _ => return root,
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return root;
    };
    // Authored and specialized state parameters retain exact symbol parents.
    // Some generated entry parameters are direct machine children. Check the
    // matching declaration roster in that owner, never every state's roster
    // in the program for each fact comparison. Names do not establish `self`.
    if program.machine_states(machine).iter().any(|state| {
        (!state_symbol.is_valid() || state.symbol == state_symbol)
            && program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self && parameter.symbol == symbol)
    }) {
        facts::PlaceRoot::Symbol(machine.symbol)
    } else {
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{machine::Machine, signature::StateParameter, state::State};

    fn program_with_self_parameter(
        machine_parent: bool,
        parameter_kind: SymbolKind,
        is_self: bool,
        stored_in_foreign_machine: bool,
    ) -> (typed_trees::TypedTrees, SymbolHandle, SymbolHandle) {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
        let machines = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Borrowed("first")),
                (SymbolKind::Machine, SymbolNameRef::Borrowed("second")),
            ],
        );
        let mut machines = SymbolTableBuilder::child_handles(machines);
        let machine_symbol = machines.next().expect("first machine");
        let foreign_machine = machines.next().expect("second machine");
        let children = symbols.insert_children(
            machine_symbol,
            std::iter::once((SymbolKind::State, SymbolNameRef::Borrowed("run")))
                .chain(machine_parent.then_some((parameter_kind, SymbolNameRef::Borrowed("self")))),
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let state_symbol = children.next().expect("state");
        let parameter = if machine_parent {
            children.next().expect("machine parameter")
        } else {
            let parameters = symbols.insert_children(
                state_symbol,
                [(parameter_kind, SymbolNameRef::Borrowed("self"))],
            );
            SymbolTableBuilder::child_handles(parameters)
                .next()
                .expect("state parameter")
        };
        let mut program = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let mut state = State {
            symbol: state_symbol,
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                is_self,
                ..Default::default()
            },
        );
        let mut machine = Machine {
            symbol: if stored_in_foreign_machine {
                foreign_machine
            } else {
                machine_symbol
            },
            ..Default::default()
        };
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        (program, machine_symbol, parameter)
    }

    #[test]
    fn self_event_roots_use_exact_state_or_machine_parent() {
        for machine_parent in [false, true] {
            let (program, machine, parameter) =
                program_with_self_parameter(machine_parent, SymbolKind::Parameter, true, false);
            assert_eq!(
                normalized_event_place_root(&program, facts::PlaceRoot::Symbol(parameter)),
                facts::PlaceRoot::Symbol(machine)
            );
        }
    }

    #[test]
    fn self_event_roots_require_live_metadata_and_owned_self_roster() {
        for (kind, is_self, foreign) in [
            (SymbolKind::Parameter, false, false),
            (SymbolKind::Local, true, false),
            (SymbolKind::Field, true, false),
            (SymbolKind::Parameter, true, true),
        ] {
            let (program, _, parameter) =
                program_with_self_parameter(false, kind, is_self, foreign);
            let root = facts::PlaceRoot::Symbol(parameter);
            assert_eq!(normalized_event_place_root(&program, root), root);
        }
        let (program, machine, parameter) =
            program_with_self_parameter(false, SymbolKind::Parameter, true, false);
        for symbol in [
            SymbolHandle::invalid(),
            machine,
            SymbolHandle::from_parts(parameter.arena_index(), parameter.generation() + 1),
        ] {
            let root = facts::PlaceRoot::Symbol(symbol);
            assert_eq!(normalized_event_place_root(&program, root), root);
        }
        assert_eq!(
            normalized_event_place_root(&program, facts::PlaceRoot::Unknown),
            facts::PlaceRoot::Unknown
        );
    }
}
