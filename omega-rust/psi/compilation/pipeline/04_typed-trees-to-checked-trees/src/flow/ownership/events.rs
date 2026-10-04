use crate::flow::CanonicalPlace;
use arena::HandleSpan;
use symbols::SymbolHandle;

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
    pub(crate) evaluation: checked_trees::FlowPermissionEvaluation,
    /// Exact evaluated operand that detaches this value. Statement receivers
    /// and production destinations have no expression; their source names the
    /// statement/call instead. Equal places at different occurrences are not
    /// interchangeable reads or moves.
    pub(crate) expression: typed_trees::expression::ExpressionHandle,
    pub(crate) source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
    pub(crate) root: facts::PlaceRoot,
    pub(crate) segments: HandleSpan<facts::PlaceSegment>,
}

pub(crate) struct DirectMoveEventSink<'segments> {
    segments: &'segments mut arena::Arena<facts::PlaceSegment>,
    pub(super) operators: &'segments checked_trees::CheckedOperatorFacts,
    pub(super) machine: &'segments typed_trees::machine::Machine,
    pub(super) state: &'segments typed_trees::state::State,
    events: Vec<DiscoveredMoveEvent>,
    proof_only: Option<std::sync::Arc<typed_trees::proof_only::ProofOnlyClassification>>,
    pub(super) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(super) source_arm: arena::Handle<typed_trees::expression::TableMatchArm>,
}

impl<'segments> DirectMoveEventSink<'segments> {
    pub(crate) fn new(
        segments: &'segments mut arena::Arena<facts::PlaceSegment>,
        operators: &'segments checked_trees::CheckedOperatorFacts,
        machine: &'segments typed_trees::machine::Machine,
        state: &'segments typed_trees::state::State,
    ) -> Self {
        Self {
            segments,
            operators,
            machine,
            state,
            events: Vec::new(),
            proof_only: None,
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            source_arm: arena::Handle::invalid(),
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
            .get_or_insert_with(|| validation::proof_only_classification(program))
    }

    fn append_move_event(
        &mut self,
        program: &typed_trees::TypedTrees,
        place: CanonicalPlace,
        source: FlowOwnershipEventSource,
        expression: typed_trees::expression::ExpressionHandle,
    ) {
        self.events.push(DiscoveredMoveEvent {
            source,
            evaluation: self.evaluation,
            expression,
            source_arm: self.source_arm,
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
    expression: typed_trees::expression::ExpressionHandle,
) {
    sink.append_move_event(program, place, source, expression);
}

/// Join a retained call ordinal to its existing authored transition lane.
/// A failed join stays unknown; it must not become a guard or ordinary event.
pub(crate) fn call_permission_evaluation(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_ordinal: usize,
) -> checked_trees::FlowPermissionEvaluation {
    use checked_trees::FlowPermissionEvaluation as Evaluation;
    match program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    {
        Some(typed_trees::statement::StatementNode::Transition(_)) => {
            match crate::semantic::calls::transition_call_target(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
            ) {
                Some(target) if target.is_valid() => Evaluation::TransitionTarget(target),
                Some(_) => Evaluation::TransitionGuard,
                None => Evaluation::Unknown,
            }
        }
        Some(_) => Evaluation::Statement,
        None => Evaluation::Unknown,
    }
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
    let Some(machine) = crate::lookup::machine_by_symbol(program, machine_symbol) else {
        return root;
    };
    // Authored and specialized state parameters retain exact symbol parents.
    // Some generated entry parameters are direct machine children. Check the
    // matching declaration roster in that owner, never every state's roster
    // in the program for each fact comparison. Names do not establish `self`.
    let owns_self_parameter = |state: &typed_trees::state::State| {
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.is_self && parameter.symbol == symbol)
    };
    // A state symbol identifies its one declaration. Reuse the existing
    // revision-aware owner lookup rather than scanning unrelated states for
    // every fact. Generated machine children keep their owner-wide fallback.
    let is_self = if state_symbol.is_valid() {
        crate::lookup::symbols::state_index_by_symbol(program, machine, state_symbol)
            .and_then(|position| program.machine_states(machine).get(position))
            .is_some_and(owns_self_parameter)
    } else {
        program
            .machine_states(machine)
            .iter()
            .any(owns_self_parameter)
    };
    if is_self {
        facts::PlaceRoot::Symbol(machine.symbol)
    } else {
        root
    }
}
