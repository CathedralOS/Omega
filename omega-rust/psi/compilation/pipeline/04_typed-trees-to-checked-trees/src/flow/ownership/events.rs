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
        place: CanonicalPlace,
        source: FlowOwnershipEventSource,
        expression: typed_trees::expression::ExpressionHandle,
    ) {
        self.events.push(DiscoveredMoveEvent {
            source,
            evaluation: self.evaluation,
            expression,
            source_arm: self.source_arm,
            root: place.root,
            segments: self.segments.insert_many(place.segments),
        });
    }
}

pub(in crate::flow::ownership) fn append_move_event_for_place(
    sink: &mut DirectMoveEventSink<'_>,
    place: CanonicalPlace,
    source: FlowOwnershipEventSource,
    expression: typed_trees::expression::ExpressionHandle,
) {
    sink.append_move_event(place, source, expression);
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
