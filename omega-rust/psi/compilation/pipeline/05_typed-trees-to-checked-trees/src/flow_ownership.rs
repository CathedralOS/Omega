/*
    Ownership move events and borrowed operand events. `discover_state_move_events` walks one
    state's statements and borrow calls and returns the moves they make, for the
    multiplicity checks and terminal cleanup. A place's declared type names
    storage; the selected operand type names the value transferred from it.
    In particular, forwarding an exclusive reference moves its descriptor,
    while a consumption of its exact referent opens restoration debt. Carry
    that distinction from local, store, call and result contexts through the
    same expression walk; a later checker must not guess it from an empty path.
    Exact place types stay with validation's value-custody projection owner.
*/

use crate::flow::canonical_place_from_symbol;
use crate::flow::state_statements;
use crate::published::places::place_at;
use checked_trees::BorrowFacts;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

pub(crate) mod calls {
    use validation::CallSite;
    use validation::CanonicalPlace;
    use validation::call_site_argument_expressions;
    use validation::call_target_parameters;
    use validation::find_call_site;

    use crate::flow::FlowOwnershipEventSource;
    use crate::flow::ownership::DirectMoveEventSink;
    use crate::flow::ownership::append_move_event_for_place;
    use crate::flow::ownership::moves;
    use crate::flow::ownership::type_requires_ownership;

    use checked_trees::BorrowCallFact;
    use checked_trees::expression::ExpressionHandle;

    pub(in crate::flow) fn append_call_ownership_events(
        program: &dyn ::validation::CheckingInput,
        sink: &mut DirectMoveEventSink<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        borrow_call: &BorrowCallFact,
    ) {
        let Some(call_site) = find_call_site(
            program,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        ) else {
            return;
        };
        if let CallSite::Expression { expression, .. } = call_site
            && validation::saved_dispatch_call(
                program,
                state,
                borrow_call.statement_index,
                expression,
            )
            .is_some_and(|(statement, canonical)| {
                statement != borrow_call.statement_index || canonical != expression
            })
        {
            return;
        }
        let arguments = call_site_argument_expressions(program, &call_site);
        let declared_parameters = call_target_parameters(program, borrow_call.target_symbol);
        let Some(declared_parameters) = declared_parameters else {
            return;
        };
        let requirement_citation = match call_site {
            CallSite::Statement(call) => {
                validation::is_fact_only_requirement_citation(program, machine.symbol, call)
            }
            _ => false,
        };
        let proof_call = requirement_citation
            || declared_parameters.iter().any(|parameter| {
                program.type_multiplicity(parameter.type_reference)
                    == language_semantics::Multiplicity::Affine
            }) && program
                .machines()
                .iter()
                .find(|target| {
                    program
                        .machine_states(target)
                        .iter()
                        .any(|target_state| target_state.symbol == borrow_call.target_symbol)
                })
                .is_some_and(|target| sink.proof_only(program).is_proof_machine(program, target));
        let parameter_transfers = |parameter: &typed_trees::signature::StateParameter| {
            type_requires_ownership(program, parameter.type_reference)
                && !(proof_call
                    && program.type_multiplicity(parameter.type_reference)
                        == language_semantics::Multiplicity::Affine
                    && !crate::checks::type_carries_linear_obligation(
                        program,
                        parameter.type_reference,
                    ))
        };

        let source = FlowOwnershipEventSource::Call {
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
            target_symbol: borrow_call.target_symbol,
        };

        let parameters = declared_parameters.iter();

        for (parameter, argument) in parameters.zip(arguments.iter()) {
            if !parameter_transfers(parameter) {
                if !parameter.is_const && !parameter.relevance.is_erased() && !proof_call {
                    moves::observations::append(
                        program,
                        sink,
                        state.symbol,
                        borrow_call.statement_index,
                        *argument,
                        FlowOwnershipEventSource::Statement {
                            statement_index: borrow_call.statement_index,
                        },
                    );
                }
                continue;
            }

            if affine_call_result_type(program, machine, state, *argument).is_some() {
                append_move_event_for_place(
                    sink,
                    CanonicalPlace {
                        root: facts::PlaceRoot::Expression(*argument),
                        segments: Vec::new(),
                    },
                    source,
                    *argument,
                    parameter.type_reference,
                );
                continue;
            }

            let operand_source = if crate::checks::owned_temporary_type(
                program,
                state.symbol,
                borrow_call.statement_index,
                parameter,
                *argument,
            )
            .is_some()
            {
                FlowOwnershipEventSource::Statement {
                    statement_index: borrow_call.statement_index,
                }
            } else {
                source
            };
            moves::append_move_events_for_expression(
                program,
                sink,
                state.symbol,
                borrow_call.statement_index,
                *argument,
                parameter.type_reference,
                operand_source,
            );
        }
    }

    fn affine_call_result_type(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let call = program.expression_table.call_node(expression)?;
        let (receiver_symbol, receiver_path) =
            validation::call_receiver_parts(program, call.receiver);
        let target = validation::resolve_state_call_target(
            program,
            machine,
            state,
            receiver_symbol,
            validation::call_target_symbol(program, call),
            receiver_path.as_deref(),
            &call.target,
        );
        let result = validation::call_target_return_type(program, target)?;
        (matches!(
            program.type_reference_table.type_reference(result),
            typed_trees::types::TypeReferenceNode::Named { .. }
                | typed_trees::types::TypeReferenceNode::Generic { .. }
                | typed_trees::types::TypeReferenceNode::FixedArray { .. }
        ) && program.type_multiplicity(result) == language_semantics::Multiplicity::Affine
            && !crate::checks::type_carries_linear_obligation(program, result))
        .then_some(result)
    }
    /*
        Canonical caller storage moved by this call's retained operand occurrences.
        Discovery ran once before claim expansion. Call-local loan checks consume
        that roster rather than reclassifying the source or rebuilding move events.
    */
    pub(crate) fn owned_call_operand_places(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        state: &checked_trees::FlowStateFact,
        borrow_call: &BorrowCallFact,
    ) -> Vec<CanonicalPlace> {
        transfers_for_source(
            facts.view(),
            state,
            FlowOwnershipEventSource::Call {
                statement_index: borrow_call.statement_index,
                call_ordinal: borrow_call.call_ordinal,
                target_symbol: borrow_call.target_symbol,
            },
        )
        .map(|event| CanonicalPlace {
            root: event.root,
            segments: facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .to_vec(),
        })
        .filter(|place| !super::result_storage::is_private_result_place(program, place))
        .collect()
    }

    /*
        The evaluated storage coordinate of one consuming operand. Mutable-reference
        extraction can move a referent while the expression still names its descriptor
        slot; reconstructing that slot from source would select a different owner.
        This association identifies storage only. Live claim authority still comes
        from the exact committed permission occurrence, not from a move candidate.
    */
    pub(crate) fn owned_operand_transfer_place(
        facts: checked_trees::CheckFactsRef<'_>,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        expression: ExpressionHandle,
        evaluation: checked_trees::FlowPermissionEvaluation,
        source: language_semantics::PermissionEventSource,
    ) -> Result<Option<CanonicalPlace>, &'static str> {
        use language_semantics::PermissionEventSource;
        facts.flow.ownership.walk_guard.note(state);
        if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
            return Err("owned operand has no permission evaluation coordinate");
        }
        let source = match source {
            PermissionEventSource::Statement { statement_index } => {
                FlowOwnershipEventSource::Statement { statement_index }
            }
            PermissionEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            } => FlowOwnershipEventSource::Call {
                statement_index,
                call_ordinal,
                target_symbol,
            },
            _ => return Ok(None),
        };
        let state = facts
            .flow
            .control
            .state(machine, state)
            .ok_or("owned operand has no state transfer roster")?;
        let mut selected = None;
        for event in transfers_for_source(facts, state, source)
            .filter(|event| event.expression == expression && event.evaluation == evaluation)
        {
            if !event.root.is_valid() {
                return Err("owned operand transfer has no evaluated storage root");
            }
            let place = CanonicalPlace {
                root: event.root,
                segments: facts
                    .flow
                    .ownership
                    .segments
                    .span(event.segments)
                    .ok_or("owned operand transfer has stale storage selectors")?
                    .to_vec(),
            };
            if selected.as_ref().is_some_and(|previous| *previous != place) {
                return Err("owned operand has ambiguous evaluated storage coordinates");
            }
            selected = Some(place);
        }
        Ok(selected)
    }

    fn transfers_for_source<'facts>(
        facts: checked_trees::CheckFactsRef<'facts>,
        state: &checked_trees::FlowStateFact,
        source: FlowOwnershipEventSource,
    ) -> impl Iterator<Item = &'facts checked_trees::FlowStorageTransferFact> {
        facts.flow.ownership.walk_guard.note(state.state_symbol);
        facts
            .flow
            .ownership
            .storage_transfers
            .span_or_empty(state.storage_transfers)
            .iter()
            .filter(move |event| event.source == source)
    }
}
mod events {
    use validation::CanonicalPlace;

    pub(crate) use checked_trees::{
        FlowStorageTransferFact as DiscoveredMoveEvent,
        FlowStorageTransferSource as FlowOwnershipEventSource,
    };

    pub(crate) struct DirectMoveEventSink<'segments> {
        segments: &'segments mut arena::PagedArenaSet<facts::PlaceSegment>,
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
            segments: &'segments mut arena::PagedArenaSet<facts::PlaceSegment>,
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
            program: &dyn ::validation::CheckingInput,
        ) -> &typed_trees::proof_only::ProofOnlyClassification {
            self.proof_only
                .get_or_insert_with(|| validation::proof_only_classification(program))
        }

        fn append_move_event(
            &mut self,
            place: CanonicalPlace,
            source: FlowOwnershipEventSource,
            expression: typed_trees::expression::ExpressionHandle,
            value_type: typed_trees::types::TypeReferenceHandle,
        ) {
            self.events.push(DiscoveredMoveEvent {
                source,
                evaluation: self.evaluation,
                expression,
                value_type,
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
        value_type: typed_trees::types::TypeReferenceHandle,
    ) {
        sink.append_move_event(place, source, expression, value_type);
    }

    /*
        Join a retained call ordinal to its existing authored transition lane.
        A failed join stays unknown; it must not become a guard or ordinary event.
    */
    pub(crate) fn call_permission_evaluation(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        call_ordinal: usize,
    ) -> checked_trees::FlowPermissionEvaluation {
        use checked_trees::FlowPermissionEvaluation as Evaluation;
        match program.statement_table.statement_at(state, statement_index) {
            Some(typed_trees::statement::StatementNode::Transition(_)) => {
                match validation::transition_call_target(
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
}
mod moves {
    use super::type_references::{OperatorResultOwnership, classify_operator_result_ownership};
    use crate::flow::FlowOwnershipEventSource;
    use crate::flow::expressions;
    use validation::ownership_place_from_expression_in_state;
    use validation::symbol_type_symbol;

    use crate::flow::canonical_place_from_symbol;
    use crate::flow::ownership::DirectMoveEventSink;
    use crate::flow::ownership::append_move_event_for_place;
    use crate::flow::ownership::type_references;
    use crate::flow::ownership::type_requires_ownership;
    use crate::flow::resolve_operator_for_call;

    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use symbols::SymbolHandle;
    pub(super) mod observations {
        use checked_trees::CheckedOperatorOccurrence;
        use checked_trees::CheckedValueOrigin;
        use facts::PlaceSegment;
        use language_semantics::Multiplicity;
        use typed_trees::operator::declaration_by_symbol;
        use typed_trees::types::TypeReferenceHandle;
        use typed_trees::types::TypeReferenceNode;
        use validation::{
            CanonicalPlace, ContentsRequirement, canonical_place_type_reference, has_contents,
        };

        use crate::flow::FlowOwnershipEventSource;

        use crate::flow::ownership::DirectMoveEventSink;
        use crate::flow::ownership::append_move_events_for_expression;
        use crate::flow::ownership::moves::operator_call_ownership_policy;
        use checked_trees::expression::{ExpressionHandle, ExpressionNode};
        use language_core::ReferenceAccess;
        use symbols::SymbolHandle;

        /* Observing a place evaluates its address and calls, without moving the place. */
        pub(in crate::flow::ownership) fn append(
            program: &dyn ::validation::CheckingInput,
            sink: &mut DirectMoveEventSink<'_>,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
            source: FlowOwnershipEventSource,
        ) {
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(_) | ExpressionNode::SavedSubject(_) => {}
                ExpressionNode::Member(member) => {
                    append(program, sink, state, statement, member.receiver, source)
                }
                ExpressionNode::Indexed(indexed) => {
                    if append_indexed_operands(program, sink, state, statement, expression, source)
                        .is_some()
                    {
                        return;
                    }
                    append(program, sink, state, statement, indexed.collection, source);
                    append_move_events_for_expression(
                        program,
                        sink,
                        state,
                        statement,
                        indexed.index,
                        Default::default(),
                        source,
                    );
                }
                ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                    append(program, sink, state, statement, cast.value, source);
                }
                ExpressionNode::Borrow(borrow) => {
                    append(program, sink, state, statement, borrow.target, source)
                }
                _ => append_move_events_for_expression(
                    program,
                    sink,
                    state,
                    statement,
                    expression,
                    Default::default(),
                    source,
                ),
            }
        }

        pub(super) enum IndexedResult {
            StoredProjection,
            FreshValue,
        }

        /*
            A selected index operator evaluates ordinary parameter-aligned operands.
            Only intrinsic place indexing may implicitly observe its collection.
            The result of a consuming collection operator is fresh, not a projection
            that could transfer that same collection a second time.
        */
        pub(super) fn append_indexed_operands(
            program: &dyn ::validation::CheckingInput,
            sink: &mut DirectMoveEventSink<'_>,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
            source: FlowOwnershipEventSource,
        ) -> Option<IndexedResult> {
            if validation::has_builtin_subslice_meaning(
                program,
                sink.machine,
                Some(sink.state),
                expression,
            ) {
                let indexed = program.expression_table.indexed_node(expression)?;
                append(program, sink, state, statement, indexed.collection, source);
                append_move_events_for_expression(
                    program,
                    sink,
                    state,
                    statement,
                    indexed.index,
                    Default::default(),
                    source,
                );
                return Some(IndexedResult::FreshValue);
            }
            let operator_symbol = selected_operator(sink, state, statement, expression)?;
            let operator = declaration_by_symbol(program, operator_symbol)?;
            let indexed = program.expression_table.indexed_node(expression)?;
            let operands = match program.expression_table.expression(indexed.index) {
                ExpressionNode::Range(range) => [indexed.collection, range.start, range.end],
                _ => [
                    indexed.collection,
                    indexed.index,
                    ExpressionHandle::invalid(),
                ],
            };
            let argument_count = if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                2
            } else {
                1
            };
            let policy =
                operator_call_ownership_policy(program, Some(operator), argument_count, true);
            for (ordinal, operand) in operands.into_iter().enumerate() {
                if !operand.is_valid() {
                    continue;
                }
                let transfers = if ordinal == 0 {
                    policy.receiver_transfers()
                } else {
                    policy.positional_transfers(ordinal - 1)
                };
                if transfers {
                    append_move_events_for_expression(
                        program,
                        sink,
                        state,
                        statement,
                        operand,
                        if ordinal == 0 {
                            policy.receiver().value_type
                        } else {
                            policy.positional(ordinal - 1).value_type
                        },
                        source,
                    );
                } else {
                    append(program, sink, state, statement, operand, source);
                }
            }
            Some(if policy.receiver_transfers() {
                IndexedResult::FreshValue
            } else {
                IndexedResult::StoredProjection
            })
        }

        pub(super) fn selected_operator(
            sink: &DirectMoveEventSink<'_>,
            state: SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            sink.operators.resolved_uses().find_map(|operator_use| {
                (operator_use.expression == expression
                    && operator_use.occurrence == CheckedOperatorOccurrence::Expression
                    && matches!(operator_use.origin, CheckedValueOrigin::StateStatement {
                        state_symbol, statement_index, ..
                    } if state_symbol == state && statement_index == statement))
                .then_some(operator_use.selected_operator_symbol)
            })
        }

        /*
            Whether one by-value use of `place` is a detached observation rather than
            an ownership transfer out of borrowed storage.

            Two reaches admit a detached copy while every other reach keeps its
            ordinary move or borrowed-window behavior:

            - A chain that crosses only shared (`&`) loans and contains an index hop
              reads the element by copy: `view[index]` on `&[T]` cannot move the
              element out of the shared loan, so element access produces a detached
              value and leaves the borrowed storage whole. The detached element
              materializes a fresh value of the element type, so every member —
              including an `[erased]` one — must hold recursively stable contents.
              Field and case projections keep their ordinary place ownership; only
              the indexed element read detaches.
            - A call to a boundary-trait requirement receives a
              caller-owned ABI copy of each stable borrowed argument: `self.foreign
              .call(self.field)` resolves to a boundary trait signature whose provider
              sits behind an opaque service handle, so it observes `self.field`
              instead of extracting it through the `&mut` receiver, and the caller's
              loan returns intact. The marshal carries runtime contents only: an
              `[erased]` member never materializes and never crosses the seam, so it
              stays with the observed place and needs only to resolve. Direct provider
              calls — `boundary machine`, `via`, admission, or requirement targets —
              still consume custody.

            A `&write` link never supplies readable content, and a `&mut` reach keeps
            its move-and-restore window outside the service-seam case, so forged or
            custody-carrying paths still reject.
        */
        pub(super) fn detached_borrowed_copy_admitted(
            program: &dyn ::validation::CheckingInput,
            state: SymbolHandle,
            statement: usize,
            place: &CanonicalPlace,
            source: FlowOwnershipEventSource,
        ) -> bool {
            let Some(_) = place.root.as_symbol() else {
                return false;
            };
            let Some(projected) = canonical_place_type_reference(program, state, statement, place)
            else {
                return false;
            };

            let mut has_reference = false;
            let mut has_exclusive = false;
            let mut has_write_only = false;
            let mut note = |access: Option<ReferenceAccess>| {
                if let Some(access) = access {
                    has_reference = true;
                    has_exclusive |= access.is_exclusive();
                    has_write_only |= access == ReferenceAccess::WriteOnly;
                }
            };

            for length in 0..place.segments.len() {
                let prefix = CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..length].to_vec(),
                };
                note(
                    canonical_place_type_reference(program, state, statement, &prefix)
                        .and_then(|reference| reference_access(program, reference)),
                );
            }
            if !has_reference || has_write_only {
                return false;
            }
            if !has_exclusive
                && place.segments.iter().any(|segment| {
                    matches!(
                        segment,
                        PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. }
                    )
                })
            {
                return program.type_multiplicity(projected) == Multiplicity::Unrestricted
                    && has_contents(
                        program,
                        projected,
                        &[],
                        ContentsRequirement::StableObservation,
                    );
            }
            let FlowOwnershipEventSource::Call { target_symbol, .. } = source else {
                return false;
            };
            call_target_is_service_signature(program, target_symbol)
                && has_contents(program, projected, &[], ContentsRequirement::SeamMarshal)
        }

        /*
            Whether `target_symbol` names a machine signature of a `boundary` trait —
            the resolved target of a boundary requirement call. The provider behind that handle is not a custody participant:
            the seam marshals a caller-owned copy of each argument, so a stable
            borrowed argument is observed, never extracted.

            Every other target keeps its transfer. A `boundary machine`, `via`
            realization, or admission claim is a provider contract: its by-value
            parameters consume custody like a checked body, so a borrowed argument
            still opens the move-and-restore window (and the boundary-call fence
            rejects an invocation that would span the hole). Requirement slots and
            machine parameters resolve to provider contracts the same way.
        */
        fn call_target_is_service_signature(
            program: &dyn ::validation::CheckingInput,
            target_symbol: SymbolHandle,
        ) -> bool {
            let signature_symbol = program
                .machine_parameter_signature(target_symbol)
                .map_or(target_symbol, |(_, signature)| signature.symbol);
            program.traits().iter().any(|definition| {
                definition.is_boundary
                    && program
                        .trait_machine_signature_by_symbol(definition, signature_symbol)
                        .is_some()
            })
        }

        /* The access of a possibly-constrained reference type. */
        fn reference_access(
            program: &dyn ::validation::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<ReferenceAccess> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { access, .. } => Some(*access),
                TypeReferenceNode::Constrained { base_type, .. } => {
                    reference_access(program, *base_type)
                }
                _ => None,
            }
        }
    }

    /*
        Select the consumed value without confusing a loan descriptor with its
        referent. Context only unwraps a reference when the admitted operand names
        the exact declared referent type; it cannot supply an unrelated result type.
        A byte view or `[T]` view is its own descriptor storage, never a pointer to
        a referent the operand could consume: storing one into a view-typed place
        (`slot.out = text`) copies the descriptor even though the assignment's
        expected type is the place's referent (`assignment_value_type`).
    */
    fn consumed_value_type(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let actual =
            validation::expression_type_reference_in_state(program, state, statement, expression)?;
        if expected.is_valid()
            && validation::stores_reference_shape(program, actual, &[])
            && let Some(shell) = validation::reference_shell(program, actual)
            && program.normalized_type_identity(shell.referee)
                == program.normalized_type_identity(expected)
        {
            return Some(expected);
        }
        Some(actual)
    }

    pub(super) fn append_move_events_for_expression(
        program: &dyn ::validation::CheckingInput,
        sink: &mut DirectMoveEventSink<'_>,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        expected_type: typed_trees::types::TypeReferenceHandle,
        source: FlowOwnershipEventSource,
    ) {
        macro_rules! append_observation {
            ($operand:expr $(,)?) => {
                observations::append(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    $operand,
                    source,
                )
            };
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && validation::applied_case_membership_variant(
                program,
                sink.machine,
                Some(sink.state),
                expression,
                binary,
            )
            .is_some()
            && observations::selected_operator(sink, state_symbol, statement_index, expression)
                .is_none()
        {
            append_observation!(binary.left);
            return;
        }

        if type_references::intrinsic_enum_equality(program, state_symbol, expression)
            || selected_meaning_borrows_operands(program, expression)
        {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                unreachable!()
            };
            for operand in [binary.left, binary.right] {
                append_observation!(operand);
            }
            return;
        }

        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Indexed(_)
        ) && matches!(
            observations::append_indexed_operands(
                program,
                sink,
                state_symbol,
                statement_index,
                expression,
                source,
            ),
            Some(observations::IndexedResult::FreshValue)
        ) {
            return;
        }

        if let Some(receiver) = validation::collection_length_receiver(
            program,
            sink.machine,
            Some(sink.state),
            expression,
        ) {
            append_observation!(receiver);
            return;
        }

        let value_type = consumed_value_type(
            program,
            state_symbol,
            statement_index,
            expression,
            expected_type,
        );
        if type_references::expression_is_place_like(program, expression)
            && value_type
                .map(|reference| {
                    type_references::stored_value_requires_transfer(program, reference)
                })
                .unwrap_or(true)
        {
            if let Some(place) = ownership_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                expression,
            ) {
                let observes_reference = value_type.is_some_and(|reference| {
                    validation::reference_shell(program, reference).is_some()
                        && (!expected_type.is_valid()
                            || !validation::declared_carriers_match(
                                program,
                                reference,
                                expected_type,
                            )
                            || reference_slot_is_borrowed(
                                program,
                                state_symbol,
                                statement_index,
                                &place,
                            ))
                });
                if observes_reference
                    || observations::detached_borrowed_copy_admitted(
                        program,
                        state_symbol,
                        statement_index,
                        &place,
                        source,
                    )
                {
                    append_observation!(expression);
                } else {
                    append_move_event_for_place(
                        sink,
                        place,
                        source,
                        expression,
                        value_type.unwrap_or_default(),
                    );
                }
            }
            return;
        }

        macro_rules! append_child {
            ($child:expr, $expected:expr $(,)?) => {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    $child,
                    $expected,
                    source,
                )
            };
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Match(dispatch) => {
                let tag_observed = validation::structural_match_subject_type(
                    program,
                    sink.machine,
                    sink.state,
                    dispatch,
                )
                .is_some();
                if !tag_observed {
                    append_child!(dispatch.subject, Default::default());
                }
                for (ordinal, arm) in expressions(program)
                    .match_arms(dispatch.arms)
                    .iter()
                    .enumerate()
                {
                    for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                        append_child!(pattern, Default::default());
                    }
                    let enclosing_arm = sink.source_arm;
                    sink.source_arm = dispatch.arms.handle_at(ordinal as u32);
                    append_child!(arm.value, expected_type);
                    sink.source_arm = enclosing_arm;
                }
            }
            ExpressionNode::Atomic(atomic) => append_child!(atomic.value, expected_type),
            ExpressionNode::ArrayLiteral(values) => {
                for value in program.expression_table.expression_handles(*values) {
                    append_child!(
                        *value,
                        if expected_type.is_valid() {
                            match program.type_reference_table.type_reference(expected_type) {
                                typed_trees::types::TypeReferenceNode::FixedArray {
                                    element_type,
                                    ..
                                } => *element_type,
                                _ => Default::default(),
                            }
                        } else {
                            Default::default()
                        },
                    );
                }
            }
            ExpressionNode::Binary(binary) => {
                append_child!(binary.left, Default::default());
                append_child!(binary.right, Default::default());
            }
            /*
                A recast re-views its operand's storage; its `target_type` names
                the presented referent, not a value read out of that reference.
                Like a recast observed in place (`observations::append`), it
                reborrows the operand rather than moving the referent out of
                borrowed storage. The loan it forms carries the exclusivity: a
                shared view may be duplicated (ownership: shared loans may
                duplicate), and a second exclusive view of a still-live one is
                refused by the borrow checker.
            */
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                append_observation!(cast.value)
            }
            ExpressionNode::Cast(cast) => append_child!(cast.value, cast.target_type),
            ExpressionNode::Unary(unary) => append_child!(unary.operand, Default::default()),
            ExpressionNode::StructLiteral(struct_literal) => {
                for field in expressions(program).struct_fields(struct_literal.fields) {
                    append_child!(
                        field.value,
                        validation::selected_construction_field_type(
                            program,
                            expression,
                            struct_literal,
                            field.field_symbol,
                        )
                        .unwrap_or_default(),
                    );
                }
            }
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    append_child!(range.start, Default::default());
                }
                if range.end.is_valid() {
                    append_child!(range.end, Default::default());
                }
            }
            ExpressionNode::Call(call) => {
                append_move_events_for_call_arguments(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    call,
                    source,
                );
            }
            ExpressionNode::Borrow(borrow) => append_observation!(borrow.target),
            ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::TypeExpression(_)
            | typed_trees::spelled_literal_expression_pattern!() => {}
        }
    }

    /*
        Evaluate nested call operands using their declared ownership policy.
        Static type/module receivers name declarations rather than runtime storage.
        A binary expression whose authored selection resolved to a machine that
        takes every operand by shared reference observes its operands, exactly as
        a call to that machine lends its arguments: `a == b` over an `Equatable`
        implementation with an authored `equals` row selects that row, and neither
        side moves.
    */
    fn selected_meaning_borrows_operands(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget as Target;
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Binary(_)
        ) {
            return false;
        }
        let mut targets = expressions(program)
            .authored_selection_occurrences(expression)
            .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
            .map(|selection| match selection.target() {
                Target::Resolved(target) => Some(target.selected_symbol()),
                _ => None,
            });
        let (Some(Some(target)), None) = (targets.next(), targets.next()) else {
            return false;
        };
        validation::call_target_parameters(program, target).is_some_and(|parameters| {
            parameters.len() == 2
                && parameters.iter().all(|parameter| {
                    matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        typed_trees::types::TypeReferenceNode::Reference {
                            access: language_semantics::ReferenceAccess::Shared,
                            ..
                        }
                    )
                })
        })
    }

    fn append_move_events_for_call_arguments(
        program: &dyn ::validation::CheckingInput,
        sink: &mut DirectMoveEventSink<'_>,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::expression::TableCallExpression,
        source: FlowOwnershipEventSource,
    ) {
        if validation::call_target_parameters(program, call.target_symbol).is_some()
            || expression_call_is_owned_by_call_flow(program, state_symbol, call)
        {
            return;
        }

        let receiver_is_static_path =
            receiver_expression_static_path_segments(program, call.receiver).is_some();
        let arguments = program.expression_table.expression_handles(call.arguments);
        let operator = resolve_operator_for_call(
            program,
            call.target_symbol,
            receiver_expression_static_path_segments(program, call.receiver).as_deref(),
            call.target.as_str(),
            arguments.len(),
            call.receiver.is_valid() && !receiver_is_static_path,
        );
        let policy = operator_call_ownership_policy(
            program,
            operator,
            arguments.len(),
            call.receiver.is_valid() && !receiver_is_static_path,
        );

        if call.receiver.is_valid() && !receiver_is_static_path && policy.receiver_transfers() {
            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                call.receiver,
                policy.receiver().value_type,
                source,
            );
        }

        for (ordinal, argument) in arguments.iter().enumerate() {
            if !policy.positional_transfers(ordinal) {
                continue;
            }

            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                *argument,
                policy.positional(ordinal).value_type,
                source,
            );
        }
    }

    /*
        Whether borrow-call discovery owns this expression call even though the
        table expression itself does not retain the final target-state symbol.

        Calls may carry an invalid `target_symbol` in the typed expression and
        resolve through the selected declaration or the receiver's exact type.
        Borrow discovery performs that resolution and emits the parameter-aligned
        moves. Expression recursion must use the same selected target or it will
        transfer those operands again instead of consuming the completed result.
    */
    fn expression_call_is_owned_by_call_flow(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        let Some(state) = program.state_by_symbol(state_symbol) else {
            return false;
        };
        let Some(machine) = program.machine_containing_state(state.symbol) else {
            return false;
        };
        let (receiver_symbol, receiver_path) =
            validation::call_receiver_parts(program, call.receiver);
        validation::resolve_state_call_target(
            program,
            machine,
            state,
            receiver_symbol,
            validation::call_target_symbol(program, call),
            receiver_path.as_deref(),
            &call.target,
        )
        .is_valid()
            || validation::receiver_can_dispatch_to_machine(
                program,
                machine,
                state,
                receiver_symbol,
                receiver_path.as_deref(),
            )
    }

    /*
        Append type-aware move events for the owned by-value arguments of a
        statement-level call that resolves to an operator/boundary definition
        (`Codec::encode(payload);`, `text.push_str(suffix);`).

        Statement calls that dispatch to a machine state are owned by the call-flow
        pass -- their discovered `BorrowCallFact`s drive
        `append_call_ownership_events` -- so this only handles operator/boundary
        targets, the one statement-call class with no borrow-call fact. The
        callee's declared parameter types decide per argument, exactly as in
        [`append_move_events_for_call_arguments`]: a copy parameter reads its
        argument by value and transfers nothing. An owned by-value `self` parameter
        consumes the receiver, so it gets a receiver-place move; the common
        borrowed receiver (`&self`/`&mut self`) transfers nothing.
    */
    pub(in crate::flow::ownership) fn append_move_events_for_operator_statement_call(
        program: &dyn ::validation::CheckingInput,
        sink: &mut DirectMoveEventSink<'_>,
        state_symbol: SymbolHandle,
        statement_index: usize,
        call: &typed_trees::statement::TableCall,
        source: FlowOwnershipEventSource,
    ) {
        let arguments = program.statement_table.expression_handles(call.arguments);
        let Some(resolved) = crate::flow::resolve_operator_statement_call(program, call) else {
            return;
        };
        let policy = operator_call_ownership_policy(
            program,
            Some(resolved.operator),
            arguments.len(),
            resolved.receiver_is_value,
        );

        if resolved.receiver_is_value
            && policy.receiver_transfers()
            && let Some(place) = canonical_place_from_symbol(call.receiver_symbol)
        {
            append_move_event_for_place(
                sink,
                place,
                source,
                Default::default(),
                policy.receiver().value_type,
            );
        }

        for (ordinal, argument) in arguments.iter().enumerate() {
            if !policy.positional_transfers(ordinal) {
                continue;
            }

            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                *argument,
                policy.positional(ordinal).value_type,
                source,
            );
        }
    }

    /*
        The receiver/argument ownership policy of one call site, derived from the
        resolved operator definition's declared parameter types. `Unknown` (no
        resolved operator) preserves the legacy behavior: every value operand
        decides by its own type through the recursive descent.
    */
    #[derive(Clone, Copy)]
    struct OperatorOperandOwnership {
        value_type: typed_trees::types::TypeReferenceHandle,
        transfers: bool,
    }

    impl OperatorOperandOwnership {
        fn unknown() -> Self {
            Self {
                value_type: Default::default(),
                transfers: true,
            }
        }
    }

    enum OperatorCallOwnershipPolicy {
        Unknown,
        Known {
            receiver: OperatorOperandOwnership,
            positional: Vec<OperatorOperandOwnership>,
        },
    }

    impl OperatorCallOwnershipPolicy {
        fn receiver(&self) -> OperatorOperandOwnership {
            match self {
                Self::Unknown => OperatorOperandOwnership::unknown(),
                Self::Known { receiver, .. } => *receiver,
            }
        }

        fn positional(&self, ordinal: usize) -> OperatorOperandOwnership {
            match self {
                Self::Unknown => OperatorOperandOwnership::unknown(),
                Self::Known { positional, .. } => positional
                    .get(ordinal)
                    .copied()
                    .unwrap_or_else(OperatorOperandOwnership::unknown),
            }
        }

        fn receiver_transfers(&self) -> bool {
            self.receiver().transfers
        }
        fn positional_transfers(&self, ordinal: usize) -> bool {
            self.positional(ordinal).transfers
        }
    }

    /*
        Line an operator's declared parameters up against a call site's receiver
        and positional arguments.

        Three shapes exist: a declared `self` parameter binds the receiver directly;
        a method-form call on a value receiver (`text.push_str(suffix)`) binds the
        declaration's leading parameter (`text: &mut String`) to the receiver and
        the rest positionally; a static-path call binds every parameter
        positionally. Each binding transfers only when its declared type owns.
    */
    fn operator_call_ownership_policy(
        program: &dyn ::validation::CheckingInput,
        operator: Option<&typed_trees::operator::OperatorDefinition>,
        argument_count: usize,
        has_value_receiver: bool,
    ) -> OperatorCallOwnershipPolicy {
        let Some(operator) = operator else {
            return OperatorCallOwnershipPolicy::Unknown;
        };

        let parameters = program.operator_parameters(operator);
        let positional: Vec<OperatorOperandOwnership> = parameters
            .iter()
            .map(|parameter| OperatorOperandOwnership {
                value_type: parameter.type_reference,
                transfers: type_requires_ownership(program, parameter.type_reference),
            })
            .collect();

        if has_value_receiver && positional.len() == argument_count + 1 {
            return OperatorCallOwnershipPolicy::Known {
                receiver: positional[0],
                positional: positional[1..].to_vec(),
            };
        }

        OperatorCallOwnershipPolicy::Known {
            receiver: OperatorOperandOwnership {
                value_type: Default::default(),
                transfers: false,
            },
            positional,
        }
    }

    /*
        The textual segments of a static type/module path receiver, or `None` when
        the receiver is absent or is a runtime value expression. A bare `Name` path
        counts as static exactly when its resolved symbol has no value type (it is
        not a local, parameter, field, or contained object).
    */
    fn receiver_expression_static_path_segments(
        program: &dyn ::validation::CheckingInput,
        receiver: ExpressionHandle,
    ) -> Option<Vec<&str>> {
        if !receiver.is_valid() {
            return None;
        }

        match program.expression_table.expression(receiver) {
            ExpressionNode::Borrow(inner) => {
                receiver_expression_static_path_segments(program, inner.target)
            }
            ExpressionNode::Name(path) => {
                let value_symbol =
                    validation::first_valid_name_path_symbol(path, &program.expression_table)
                        .filter(|symbol| symbol_type_symbol(program, *symbol).is_some());
                if value_symbol.is_some() {
                    return None;
                }
                Some(
                    expressions(program)
                        .name_path_members(path.members)
                        .iter()
                        .map(|identifier| identifier.as_str())
                        .collect(),
                )
            }
            _ => None,
        }
    }

    /*
        Whether a `let`-initializer expression produces a freshly owned value through
        a boundary/slice/string/collection operator call (as opposed to borrowing a
        view or reading an existing place).

        Used to decide whether an owned binding's initializer warrants a move event
        into the bound local. Operator results are classified through
        [`classify_operator_result_ownership`], keeping operator-result ownership
        policy in one place; a place-like initializer is excluded because its
        transfer is already recorded by the source-side move.
    */
    pub(in crate::flow::ownership) fn initializer_produces_owned_value(
        program: &dyn ::validation::CheckingInput,
        initializer: ExpressionHandle,
    ) -> bool {
        if !initializer.is_valid() {
            return false;
        }

        let call = match program.expression_table.expression(initializer) {
            ExpressionNode::Call(call) => call,
            _ => return false,
        };

        let static_segments = receiver_expression_static_path_segments(program, call.receiver);
        let arguments = program.expression_table.expression_handles(call.arguments);
        resolve_operator_for_call(
            program,
            call.target_symbol,
            static_segments.as_deref(),
            call.target.as_str(),
            arguments.len(),
            call.receiver.is_valid() && static_segments.is_none(),
        )
        .map(|operator| {
            classify_operator_result_ownership(program, operator.return_type)
                == OperatorResultOwnership::OwnedValue
        })
        .unwrap_or(false)
    }

    /*
        Only a positively known borrowed boundary suppresses a descriptor move.
        Unknown geometry retains the candidate for ordinary ownership validation;
        the final slot is the carried descriptor, not a crossed reference.
    */
    fn reference_slot_is_borrowed(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        place: &validation::CanonicalPlace,
    ) -> bool {
        let mut prefix = validation::CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        };
        let Some(root) =
            validation::canonical_place_type_reference(program, state, statement, &prefix)
        else {
            return false;
        };
        if validation::reference_shell(program, root).is_some()
            && place.root.as_symbol().is_some_and(|symbol| {
                program
                    .state_by_symbol(state)
                    .is_some_and(|state| program.state_parameter_by_symbol(state, symbol).is_some())
            })
        {
            return true;
        }
        for segment in &place.segments {
            let Some(reference) =
                validation::canonical_place_type_reference(program, state, statement, &prefix)
            else {
                return false;
            };
            if validation::reference_shell(program, reference).is_some() {
                return true;
            }
            prefix.segments.push(*segment);
        }
        false
    }
}
mod result_storage {
    /*
        Storage contained in a by-value call result. This does not classify the
        origins of references stored in that result or discharge its permissions.
    */
    use validation::CanonicalPlace;
    use validation::project_type_reference_from_segments;

    use typed_trees::data::DataMember;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(super) fn is_private_result_place(
        program: &dyn ::validation::CheckingInput,
        place: &CanonicalPlace,
    ) -> bool {
        let Some(expression) = place.root.as_expression() else {
            return false;
        };
        let Some(call) = program.expression_table.call_node(expression) else {
            return false;
        };
        let Some(result) = validation::call_target_return_type(program, call.target_symbol) else {
            return false;
        };
        let Some(root_type) = unconstrained_type(program, result) else {
            return false;
        };
        match program.type_reference_table.type_reference(root_type) {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => {
                if !symbol.is_valid()
                    || program
                        .data_definitions()
                        .iter()
                        .filter(|definition| definition.symbol == *symbol)
                        .count()
                        != 1
                {
                    return false;
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. } if element_type.is_valid() => {}
            _ => return false,
        }
        let mut current = result;
        let mut selected_variant = None;
        for (index, segment) in place.segments.iter().enumerate() {
            let Some(reference) = unconstrained_type(program, current) else {
                return false;
            };
            match (
                program.type_reference_table.type_reference(reference),
                segment,
            ) {
                (
                    TypeReferenceNode::Named { symbol, .. }
                    | TypeReferenceNode::Generic {
                        base_symbol: symbol,
                        ..
                    },
                    facts::PlaceSegment::Case { .. } | facts::PlaceSegment::Field { .. },
                ) => {
                    let mut definitions = program
                        .data_definitions()
                        .iter()
                        .filter(|definition| symbol.is_valid() && definition.symbol == *symbol);
                    let Some(definition) = definitions.next() else {
                        return false;
                    };
                    if definitions.next().is_some() {
                        return false;
                    }
                    let members = program.data_members(definition);
                    match segment {
                        facts::PlaceSegment::Case { variant } => {
                            if selected_variant.is_some() || !members.iter().any(|member| {
                                matches!(member, DataMember::Variant(candidate) if candidate.symbol == *variant)
                            }) {
                                return false;
                            }
                            selected_variant = Some(*variant);
                            continue;
                        }
                        facts::PlaceSegment::Field { symbol } => {
                            if !symbol.is_valid()
                                || !members.iter().any(|member| match member {
                                    DataMember::Field(field) => {
                                        selected_variant.is_none() && field.symbol == *symbol
                                    }
                                    DataMember::Variant(variant) => {
                                        selected_variant == Some(variant.symbol)
                                            && program
                                                .data_payload_fields(variant)
                                                .iter()
                                                .any(|field| field.symbol == *symbol)
                                    }
                                })
                            {
                                return false;
                            }
                            selected_variant = None;
                        }
                        _ => unreachable!(),
                    }
                }
                (
                    TypeReferenceNode::FixedArray { .. },
                    facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. },
                ) if selected_variant.is_none() => {}
                _ => return false,
            }
            let Some(projected) =
                project_type_reference_from_segments(program, result, &place.segments[..=index])
            else {
                return false;
            };
            current = projected;
        }
        selected_variant.is_none() && current.is_valid()
    }

    fn unconstrained_type(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        reference.is_valid().then_some(reference)
    }
}
mod type_references {
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use language_semantics::Multiplicity;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::data::DataMember;
    use typed_trees::expression::BinaryOperator;
    use typed_trees::operator::has_builtin_spelled_expression_meaning;
    use typed_trees::types::TypeReferenceHandle;
    use validation::expression_result_type_reference;
    use validation::unique_data_definition_by_symbol;

    /*
        The ownership disposition of a value produced by a slice/string/collection
        operator result.

        This is the seam the ownership lane uses to keep operator semantics in one
        place and to host a future user-defined copy/drop policy: today the result
        of a view operator borrows, the result of a conversion owns, and an in-place
        mutation produces no transferable value. A later policy lookup (per element
        type, per operator) plugs in here without touching the event emitters.
    */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(in crate::flow::ownership) enum OperatorResultOwnership {
        /*
            The result is a borrowed view into storage owned elsewhere
            (subslice/`Str::range`, ...).
        */
        BorrowedView,
        /*
            The result is freshly owned storage that must be moved/dropped
            (`Vec::with_capacity`, `String::with_capacity`, ...).
        */
        OwnedValue,
        /*
            The result carries no transferable ownership: either unit (an in-place
            mutation like `Vec::push`/`String::push_str`) or a copy primitive
            (`Slice::Length`, `Slice::index` of a scalar, ...).
        */
        NoTransfer,
    }

    /*
        Classify the ownership disposition of a boundary/slice/string operator's
        *result*, given the operator's return type.

        The return type is authoritative for the three built-in dispositions:
        a reference return is a borrowed view, an owned aggregate return is owned,
        and unit / copy-primitive returns transfer nothing. This mirrors
        [`type_requires_ownership`] but is expressed in operator-result terms so the
        move/borrow/drop emitters and a future copy/drop policy share one decision.
    */
    pub(in crate::flow::ownership) fn classify_operator_result_ownership(
        program: &dyn ::validation::CheckingInput,
        return_type: TypeReferenceHandle,
    ) -> OperatorResultOwnership {
        if !return_type.is_valid() {
            return OperatorResultOwnership::NoTransfer;
        }

        match program.type_reference_table.type_reference(return_type) {
            typed_trees::types::TypeReferenceNode::Reference { .. } => {
                OperatorResultOwnership::BorrowedView
            }
            typed_trees::types::TypeReferenceNode::ConstExpression(_)
            | typed_trees::types::TypeReferenceNode::Unit => OperatorResultOwnership::NoTransfer,
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                classify_operator_result_ownership(program, *base_type)
            }
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::DynamicTrait { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::Generic { .. }
            | typed_trees::types::TypeReferenceNode::Named { .. } => {
                if type_requires_ownership(program, return_type) {
                    OperatorResultOwnership::OwnedValue
                } else {
                    OperatorResultOwnership::NoTransfer
                }
            }
        }
    }

    pub(in crate::flow::ownership) fn type_requires_ownership(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        type_reference.is_valid()
            && program.type_multiplicity(type_reference) != Multiplicity::Unrestricted
    }

    /*
        Transfer discipline of a value stored into an owned receiving home.
        Reference call formals instead lend ingress, using source-use multiplicity.
        A shared carrier copies; an exclusive stored carrier moves independently
        of its referent and of the reusable spelling of a reference expression.
    */
    pub(super) fn stored_value_requires_transfer(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        reference.is_valid()
            && program.stored_value_multiplicity(reference) != Multiplicity::Unrestricted
    }

    pub(super) fn expression_is_place_like(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        if !expression.is_valid() {
            return false;
        }

        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(_) => false,
            ExpressionNode::Borrow(_) => false,
            ExpressionNode::SavedSubject(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_) => true,
            ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Call(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Range(_)
            | ExpressionNode::String(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => false,
        }
    }

    /*
        Equality observes the tags of an exact payload-free nominal sum. The shared
        typed classifier excludes authored and selected operator meanings first.
    */
    pub(super) fn intrinsic_enum_equality(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        expression: ExpressionHandle,
    ) -> bool {
        use typed_trees::types::TypeReferenceNode;
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        let spelling = match binary.operator {
            BinaryOperator::Equal => language_core::operator_spelling::OperatorSpelling::Equal,
            BinaryOperator::NotEqual => {
                language_core::operator_spelling::OperatorSpelling::NotEqual
            }
            _ => return false,
        };
        let machine_symbol = program.symbols.get(state).parent;
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            return false;
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == state)
        else {
            return false;
        };
        if program.symbols.get(state.symbol).kind != SymbolKind::State
            || program.symbols.get(machine.symbol).kind != SymbolKind::Machine
        {
            return false;
        }
        let operands = [binary.left, binary.right]
            .map(|operand| expression_result_type_reference(program, machine, state, operand));
        let nominal = |reference: Option<TypeReferenceHandle>| {
            let mut reference = reference?;
            let mut seen = Vec::new();
            while !seen.contains(&reference) {
                seen.push(reference);
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                    TypeReferenceNode::Named { symbol, .. } => return Some(*symbol),
                    _ => return None,
                }
            }
            None
        };
        let Some(symbol) = nominal(operands[0]) else {
            return false;
        };
        if nominal(operands[1]) != Some(symbol) {
            return false;
        }
        let Some(data) = unique_data_definition_by_symbol(program, symbol) else {
            return false;
        };
        let members = program.data_members(data);
        if members.is_empty() || !members.iter().all(|member| matches!(member, DataMember::Variant(variant) if program.data_payload_fields(variant).is_empty())) { return false; }
        has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &operands,
        )
    }
}

use calls::append_call_ownership_events;
pub(crate) use calls::{owned_call_operand_places, owned_operand_transfer_place};
use events::{DirectMoveEventSink, append_move_event_for_place};
pub(crate) use events::{
    DiscoveredMoveEvent, FlowOwnershipEventSource, call_permission_evaluation,
};
use moves::{
    append_move_events_for_expression, append_move_events_for_operator_statement_call,
    initializer_produces_owned_value,
};
use type_references::type_requires_ownership;

pub(super) fn append_statement_ownership_events(
    program: &dyn ::validation::CheckingInput,
    sink: &mut DirectMoveEventSink<'_>,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) {
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            let stored_type = validation::find_state(program, state_symbol)
                .and_then(|(machine, state)| {
                    validation::assignment_stored_value_type(program, machine, state, assignment)
                })
                .unwrap_or_default();
            append_move_events_for_expression(
                program,
                sink,
                state_symbol,
                statement_index,
                assignment.value,
                stored_type,
                source,
            );
            if initializer_produces_owned_value(program, assignment.value)
                && let Some(place) =
                    place_at(program, state_symbol, statement_index, assignment.target)
            {
                append_move_event_for_place(
                    sink,
                    place,
                    source,
                    Default::default(),
                    Default::default(),
                );
            }
        }
        StatementNode::LocalData(local_data) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            if !type_references::stored_value_requires_transfer(program, local_data.type_reference)
            {
                moves::observations::append(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                    source,
                );
            } else {
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    local_data.initial_value,
                    local_data.type_reference,
                    source,
                );
                if initializer_produces_owned_value(program, local_data.initial_value)
                    && let Some(place) = canonical_place_from_symbol(local_data.symbol)
                {
                    append_move_event_for_place(
                        sink,
                        place,
                        source,
                        Default::default(),
                        local_data.type_reference,
                    );
                }
            }
        }
        StatementNode::Transition(transition) => {
            let source = FlowOwnershipEventSource::Statement { statement_index };
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                sink.evaluation = checked_trees::FlowPermissionEvaluation::TransitionGuard;
                append_move_events_for_expression(
                    program,
                    sink,
                    state_symbol,
                    statement_index,
                    guard,
                    Default::default(),
                    source,
                );
            }
            for handle in transition.targets() {
                sink.evaluation = checked_trees::FlowPermissionEvaluation::TransitionTarget(handle);
                if let typed_trees::statement::TransitionTargetNode::Value(value) =
                    program.statement_table.transition_target(handle)
                {
                    append_move_events_for_expression(
                        program,
                        sink,
                        state_symbol,
                        statement_index,
                        *value,
                        sink.state.return_type,
                        source,
                    );
                }
            }
        }
        StatementNode::Call(call) => append_move_events_for_operator_statement_call(
            program,
            sink,
            state_symbol,
            statement_index,
            call,
            FlowOwnershipEventSource::Statement { statement_index },
        ),
        StatementNode::Expression(expression) => append_move_events_for_expression(
            program,
            sink,
            state_symbol,
            statement_index,
            *expression,
            if statement_index + 1 == state_statements(program, sink.state.statement_nodes).len() {
                sink.state.return_type
            } else {
                Default::default()
            },
            FlowOwnershipEventSource::Statement { statement_index },
        ),
    }
}

/*
    Discover storage-transfer candidates once in admitted execution order.
    Permission production retains this roster before expanding claims, so loan
    checks and cleanup share exact source occurrences even for claim-free values.
*/
pub(crate) fn discover_state_move_events(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    operators: &checked_trees::CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    segments: &mut arena::PagedArenaSet<facts::PlaceSegment>,
) -> Vec<DiscoveredMoveEvent> {
    let mut sink = DirectMoveEventSink::new(segments, operators, machine, state);
    let borrow_calls = borrow
        .first_state_borrow(machine.symbol, state.symbol)
        .map(|(_, state)| borrow.calls.span_or_empty(state.calls))
        .unwrap_or_default();
    let mut call_index = 0usize;

    for (statement_index, statement) in program
        .statement_table
        .admitted_execution_statements(state.statement_nodes)
    {
        sink.evaluation = checked_trees::FlowPermissionEvaluation::Statement;
        append_statement_ownership_events(
            program,
            &mut sink,
            state.symbol,
            statement_index,
            statement,
        );
        while let Some(call) = borrow_calls.get(call_index) {
            if call.statement_index != statement_index {
                break;
            }
            call_index += 1;
            sink.evaluation = call_permission_evaluation(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            );
            append_call_ownership_events(program, &mut sink, machine, state, call);
        }
    }

    sink.finish()
}
