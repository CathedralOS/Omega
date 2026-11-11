/*
    Chapter 5 execution order for statement and transition expression effects.

    Borrow call ordinals identify authored occurrences; their preorder is not
    execution order. This traversal owns invocation scheduling, not arithmetic
    interpretation or callee execution.
    Closed Boolean results reuse selected scalar evaluation after child effects
    have been scheduled. Otherwise flow retains calls that computation folding
    skips, breaking their exact occurrence correspondence. Runtime storage facts
    remain tied to the existing live contexts, never a second operand evaluation.
    Stores bind their destination carrier before evaluating children and retain
    that completed tree; later transfers must not reconstruct it from syntax.
    Indexed addresses retain each selector's own completion facts. A produced
    empty referent span is unresolved evidence, not permission to reconstruct
    an address from an initializer or later selector value.
*/

use validation::CallSite;
use validation::CanonicalPlace;

use crate::flow::FlowBuildContext;
use crate::flow::build_call_flow_fact;
use crate::flow::canonical_place_segments_may_overlap;
use crate::flow::filter_contexts_after_place_mutations;
use crate::flow::project_constraint_refs_to_active_contexts;
use crate::flow::reference_spans;
use crate::published::places::place_at;

use arena::{Handle, HandleSpan};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintRef,
    FlowInvalidationSource, FlowSemanticContextRef, ProofFacts,
};
use facts::FactPlan;
use typed_trees::expression::{BinaryOperator, UnaryOperator};
use typed_trees::statement::{TableTransition, TransitionTargetHandle, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

mod boolean_completions {
    /*
        A Boolean completion retains its possible evaluation continuations.
        An enclosing selective operand consumes the selected continuation before
        evaluating its right child; ordinary value completion still meets both.
        These are evaluation-local scratch handles, not expression-keyed facts or
        permission to rerun a call. Each continuation retains the exact captured
        descriptor inventory, constraints and write prefix of its actual route.
    */
    use super::{EvaluatedOperand, EvaluatedWritePrefix, Execution};
    use crate::flow::GuardObservation;
    use arena::{Handle, HandleSpan};
    use checked_trees::{FlowConstraintRef, FlowOperandReferent, FlowSemanticContextRef};
    use typed_trees::expression::{BinaryOperator, TableBinaryExpression};

    #[derive(Clone, Copy)]
    struct BooleanContinuation {
        writes: EvaluatedWritePrefix,
        contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: HandleSpan<FlowConstraintRef>,
    }

    #[derive(Clone, Copy, Default)]
    pub(super) struct BooleanCompletions {
        when_true: Option<BooleanContinuation>,
        when_false: Option<BooleanContinuation>,
    }

    impl BooleanCompletions {
        fn selected(self, value: bool) -> Option<BooleanContinuation> {
            if value {
                self.when_true
            } else {
                self.when_false
            }
        }

        fn known_value(self) -> Option<bool> {
            match (self.when_true.is_some(), self.when_false.is_some()) {
                (true, false) => Some(true),
                (false, true) => Some(false),
                _ => None,
            }
        }
    }

    impl Execution<'_, '_, '_> {
        pub(super) fn negated_boolean_completion(
            &mut self,
            completion: Handle<BooleanCompletions>,
        ) -> Handle<BooleanCompletions> {
            if !completion.is_valid() {
                return completion;
            }
            let original = *self.boolean_completions.get(completion);
            self.boolean_completions.insert(BooleanCompletions {
                when_true: original.when_false,
                when_false: original.when_true,
            })
        }

        fn boolean_paths(
            &self,
            completion: Handle<BooleanCompletions>,
            known: Option<bool>,
            contexts: HandleSpan<FlowSemanticContextRef>,
            constraints: HandleSpan<FlowConstraintRef>,
        ) -> BooleanCompletions {
            if completion.is_valid() {
                let mut completed = *self.boolean_completions.get(completion);
                if known == Some(true) {
                    completed.when_false = None;
                }
                if known == Some(false) {
                    completed.when_true = None;
                }
                return completed;
            }
            let continuation = BooleanContinuation {
                writes: self.write_prefix(),
                contexts,
                constraints,
            };
            BooleanCompletions {
                when_true: (known != Some(false)).then_some(continuation),
                when_false: (known != Some(true)).then_some(continuation),
            }
        }

        fn select_boolean_path(
            &mut self,
            continuation: BooleanContinuation,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            self.storage_snapshot = continuation.writes.storage_snapshot;
            self.entry_origin_writes = continuation.writes.writes;
            self.entry_origin_unknown_write = continuation.writes.unknown;
            self.entry_binding_writes = continuation.writes.binding_writes;
            self.entry_binding_unknown_write = continuation.writes.binding_unknown;
            *contexts = continuation.contexts;
            *constraints = continuation.constraints;
        }

        fn join_mutation_places(
            &mut self,
            first: HandleSpan<FlowOperandReferent>,
            second: HandleSpan<FlowOperandReferent>,
        ) -> HandleSpan<FlowOperandReferent> {
            if first == second {
                return first;
            }
            let mut places = self
                .context
                .control
                .operand_origin_writes
                .span_or_empty(first)
                .to_vec();
            for place in self
                .context
                .control
                .operand_origin_writes
                .span_or_empty(second)
            {
                if !places.contains(place) {
                    places.push(*place);
                }
            }
            self.context
                .control
                .operand_origin_writes
                .insert_many(places)
        }

        fn join_boolean_paths(
            &mut self,
            first: Option<BooleanContinuation>,
            second: Option<BooleanContinuation>,
        ) -> Option<BooleanContinuation> {
            let (first, second) = match (first, second) {
                (Some(first), Some(second)) => (first, second),
                (one, None) | (None, one) => return one,
            };
            self.join_storage_prefixes(&[first.writes, second.writes]);
            let mut contexts = second.contexts;
            let mut constraints = second.constraints;
            self.meet(
                first.contexts,
                first.constraints,
                &mut contexts,
                &mut constraints,
            );
            Some(BooleanContinuation {
                writes: EvaluatedWritePrefix {
                    length: self.operand_writes.len(),
                    storage_snapshot: self.storage_snapshot,
                    writes: self.join_mutation_places(first.writes.writes, second.writes.writes),
                    unknown: first.writes.unknown || second.writes.unknown,
                    binding_writes: self.join_mutation_places(
                        first.writes.binding_writes,
                        second.writes.binding_writes,
                    ),
                    binding_unknown: first.writes.binding_unknown || second.writes.binding_unknown,
                },
                contexts,
                constraints,
            })
        }

        pub(super) fn logical_expression(
            &mut self,
            binary: &TableBinaryExpression,
            evaluated_left: EvaluatedOperand,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) -> (
            Option<bool>,
            Handle<BooleanCompletions>,
            HandleSpan<checked_trees::FlowOperandFact>,
            Option<usize>,
        ) {
            let evaluate_when = binary.operator == BinaryOperator::And;
            let left = self.boolean_paths(
                evaluated_left.boolean_completion,
                evaluated_left.boolean,
                *contexts,
                *constraints,
            );
            let skipped = left.selected(!evaluate_when);
            let conditional_inputs_from = skipped.is_some().then_some(1);
            let Some(selected) = left.selected(evaluate_when) else {
                let inputs = self
                    .context
                    .control
                    .operands
                    .insert_many([evaluated_left.capture]);
                return (
                    left.known_value(),
                    self.boolean_completions.insert(left),
                    inputs,
                    None,
                );
            };
            self.select_boolean_path(selected, contexts, constraints);
            super::super::exits::append_predicate_context(
                self.program,
                self.semantic,
                self.context,
                self.state.symbol,
                self.statement_index,
                GuardObservation::evaluated(binary.left, evaluate_when, self.statement_index),
                facts::ProgramPoint::Statement {
                    machine_symbol: self.machine.symbol,
                    state_symbol: self.state.symbol,
                    statement_index: self.statement_index,
                },
                selected.writes.writes,
                selected.writes.unknown,
                contexts,
                constraints,
            );
            let evaluated_right = self.evaluated_operand(binary.right, contexts, constraints);
            let right = self.boolean_paths(
                evaluated_right.boolean_completion,
                evaluated_right.boolean,
                *contexts,
                *constraints,
            );
            let completions = if evaluate_when {
                BooleanCompletions {
                    when_true: right.when_true,
                    when_false: self.join_boolean_paths(skipped, right.when_false),
                }
            } else {
                BooleanCompletions {
                    when_true: self.join_boolean_paths(skipped, right.when_true),
                    when_false: right.when_false,
                }
            };
            if let Some(joined) =
                self.join_boolean_paths(completions.when_true, completions.when_false)
            {
                self.select_boolean_path(joined, contexts, constraints);
            }
            (
                completions.known_value(),
                self.boolean_completions.insert(completions),
                self.context
                    .control
                    .operands
                    .insert_many([evaluated_left.capture, evaluated_right.capture]),
                conditional_inputs_from,
            )
        }
    }
}
mod call_storage {
    /*
        Project body-derived call storage onto actual evaluated operands. Both
        installed slots and returned descriptors use this operation; neither may
        replay an initializer or confuse a descriptor cell with its pointee.
    */
    use super::Execution;
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{FlowOperandFact, FlowStorageSnapshot};
    use facts::PlaceSegment;
    use typed_trees::types::TypeReferenceHandle;
    use validation::LocalWriteSource;
    use validation::collection_element_type_reference;
    use validation::completed_storage::reference_captures_at_path;
    use validation::completed_storage::{StoredOperand, reference_sources_at_path};
    use validation::project_type_reference_from_segments;
    use validation::reference_shell;
    use validation::storage_origin_types_match;

    pub(super) struct CallPlaceProjection {
        pub(super) addresses: Vec<super::super::CapturedReferencePlace>,
        pub(super) type_reference: TypeReferenceHandle,
        sources: StoredOperand,
        projection: Vec<PlaceSegment>,
    }

    fn stored_operand_is_incoming(source: &StoredOperand) -> bool {
        match source {
            StoredOperand::Incoming(_) => true,
            StoredOperand::Alternatives(sources) => {
                !sources.is_empty() && sources.iter().all(stored_operand_is_incoming)
            }
            StoredOperand::Unavailable | StoredOperand::Value { .. } => false,
        }
    }

    impl Execution<'_, '_, '_> {
        /*
            Stop at the descriptor slot. Only intermediate reference crossings
            load descriptors, and each such load uses the same pre-call snapshot.
        */
        pub(super) fn project_call_place(
            &mut self,
            actual: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            at: Handle<FlowStorageSnapshot>,
        ) -> Option<CallPlaceProjection> {
            let captured_actual = actual;
            let actual = *self.context.control.operands.get(actual);
            let mut sources = StoredOperand::Value {
                capture: captured_actual,
                reference: actual.type_reference,
                projection: Vec::new(),
                slot: None,
            };
            let mut descriptor_end = 0;
            let mut reference = actual.type_reference;
            let mut places = if let Some(shell) = reference_shell(self.program, reference) {
                reference = shell.referee;

                self.captured_reference_places(actual.referents)
            } else {
                vec![super::super::CapturedReferencePlace::unformed(place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    actual.expression,
                )?)]
            };
            for (ordinal, segment) in path.iter().enumerate() {
                if matches!(
                    segment,
                    PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange
                ) {
                    collection_element_type_reference(self.program, reference)?;
                } else {
                    reference = project_type_reference_from_segments(
                        self.program,
                        reference,
                        std::slice::from_ref(segment),
                    )?;
                }
                places = places
                    .into_iter()
                    .map(|place| place.projected(std::slice::from_ref(segment)))
                    .collect::<Option<Vec<_>>>()?;
                if ordinal + 1 < path.len()
                    && let Some(shell) = reference_shell(self.program, reference)
                {
                    reference = shell.referee;
                    sources = reference_sources_at_path(
                        self.program,
                        &self.context.control,
                        captured_actual,
                        &path[..=ordinal],
                        at,
                    );
                    descriptor_end = ordinal + 1;
                    places = super::super::stored_views::reference_places_at_snapshot(
                        self.program,
                        &self.context.control,
                        captured_actual,
                        &path[..=ordinal],
                        at,
                        |place| self.is_incoming_descriptor_place(place),
                    )
                    .unwrap_or_default();
                }
            }
            Some(CallPlaceProjection {
                addresses: places,
                type_reference: reference,
                sources,
                projection: path[descriptor_end..].to_vec(),
            })
        }

        pub(super) fn call_backing_values(
            &mut self,
            actual: Handle<FlowOperandFact>,
            source: &LocalWriteSource,
            destination: TypeReferenceHandle,
            at: Handle<FlowStorageSnapshot>,
        ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            let actual_value = *self.context.control.operands.get(actual);
            let desired = reference_shell(self.program, destination)?;
            if ::validation::trace_flags::check_sites() {
                eprintln!(
                    "CHECKSITE call-backing-value-input state={:?} statement={} actual={actual:?} actual_type={:?} destination={destination:?} source_root={:?} source_path={:?} recasts={:?} representation={:?}",
                    self.state.symbol,
                    self.statement_index,
                    actual_value.type_reference,
                    source.root,
                    source.segments,
                    source.recasts,
                    source.representation
                );
            }
            if source.recasts.is_empty() && source.representation.is_none() {
                if source.segments.is_empty()
                    && reference_shell(self.program, actual_value.type_reference).is_some()
                {
                    return self.call_slot_values(actual, &[], at);
                }
                let projected = self.project_call_place(actual, &source.segments, at)?;
                let window = source.segments.last().is_some_and(|segment| {
                    matches!(
                        segment,
                        PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange
                    )
                });
                if !window && reference_shell(self.program, projected.type_reference).is_some() {
                    return self.call_slot_values(actual, &source.segments, at);
                }
                if window && stored_operand_is_incoming(&projected.sources) {
                    return self.incoming_reference_window(
                        actual,
                        &source.segments,
                        &projected.projection,
                        destination,
                        at,
                    );
                }
                let (actual_type, expected_type) = if window {
                    (
                        collection_element_type_reference(self.program, projected.type_reference)?,
                        collection_element_type_reference(self.program, desired.referee)?,
                    )
                } else {
                    (projected.type_reference, desired.referee)
                };
                if !storage_origin_types_match(self.program, actual_type, expected_type) {
                    return None;
                }
                let referents = self.append_captured_references(&projected.addresses);
                let sources = projected.sources.captures()?;
                if sources.is_empty() {
                    return None;
                }
                let mut roots = Vec::new();
                for source in &sources {
                    let value = self.context.control.operands.get(source.capture);
                    let profiles =
                        validation::completed_storage::captured_retention_profiles_at_path(
                            self.program,
                            &self.context.control,
                            value,
                            &source.projection,
                        )?;
                    if profiles.is_empty() {
                        return None;
                    }
                    for profile in profiles {
                        for root in self.context.control.retained_roots(profile)? {
                            crate::shared::collections::push_unique(&mut roots, *root);
                        }
                    }
                }
                let retention = self.append_retention(&roots);
                let reborrow_sources = sources
                    .into_iter()
                    .map(|source| checked_trees::FlowOperandReborrowSource {
                        operand: source.capture,
                        source_path: self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(source.projection),
                        projection: self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(projected.projection.iter().copied()),
                    })
                    .collect::<Vec<_>>();
                let reborrow_sources = self
                    .context
                    .control
                    .operand_reborrow_sources
                    .insert_many(reborrow_sources);
                if self.context.control.operand_retentions.is_valid(retention) {
                    self.context
                        .control
                        .operand_retentions
                        .get_mut(retention)
                        .reborrow_sources = reborrow_sources;
                }
                let value = self.context.control.operands.insert(FlowOperandFact {
                    type_reference: destination,
                    state_symbol: self.state.symbol,
                    statement_index: self.statement_index,
                    storage_snapshot: at,
                    referents,
                    retention,
                    ..Default::default()
                });
                if ::validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE call-backing-value-created kind=reborrow state={:?} statement={} value={value:?} actual={actual:?} retention={retention:?} reborrow_sources={reborrow_sources:?}",
                        self.state.symbol, self.statement_index
                    );
                }
                return Some(vec![(value, Vec::new())]);
            }
            let roots = super::super::operand_values::captured_lifetime_roots(
                self.program,
                &self.context.control,
                &actual_value,
            )?;
            let retention = self.append_retention(&roots);
            let places = Vec::new();
            let referents = self.append_referent_places(&places);
            let value = self.context.control.operands.insert(FlowOperandFact {
                type_reference: destination,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                storage_snapshot: at,
                referents,
                retention,
                ..Default::default()
            });
            if ::validation::trace_flags::check_sites() {
                eprintln!(
                    "CHECKSITE call-backing-value-created kind=presentation state={:?} statement={} value={value:?} actual={actual:?} retention={retention:?}",
                    self.state.symbol, self.statement_index
                );
            }
            Some(vec![(value, Vec::new())])
        }

        pub(super) fn call_slot_values(
            &mut self,
            actual: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            at: Handle<FlowStorageSnapshot>,
        ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            let actual_value = *self.context.control.operands.get(actual);
            if project_type_reference_from_segments(self.program, actual_value.type_reference, path)
                .is_some_and(|reference| reference_shell(self.program, reference).is_some())
            {
                if let Some(stored) = reference_captures_at_path(
                    self.program,
                    &self.context.control,
                    actual,
                    path,
                    at,
                ) {
                    return Some(
                        stored
                            .captures()?
                            .into_iter()
                            .map(|selected| {
                                (
                                    self.withhold_projected_pointer(
                                        selected.capture,
                                        &selected.projection,
                                    ),
                                    selected.projection,
                                )
                            })
                            .collect(),
                    );
                }
                return Some(vec![(
                    self.withhold_projected_pointer(actual, path),
                    path.to_vec(),
                )]);
            }
            if path.is_empty() {
                return Some(vec![(actual, Vec::new())]);
            }
            if reference_shell(self.program, actual_value.type_reference).is_none()
                && let Some(selected) =
                    super::super::operand_values::captured_operand_handle_at_path(
                        self.program,
                        &self.context.control,
                        actual,
                        actual_value.type_reference,
                        path,
                    )
            {
                return Some(vec![(selected, Vec::new())]);
            }
            let projected = self.project_call_place(actual, path, at)?;
            let mut values = Vec::new();
            for place in projected.addresses {
                let stored = super::super::stored_values::at_snapshot(
                    self.program,
                    &self.context.control,
                    at,
                    &place.place,
                    super::super::stored_values::StoredObservation::Binding,
                )?
                .captures()?;
                for selected in stored {
                    values.push((selected.capture, selected.projection));
                }
            }
            (!values.is_empty()).then_some(values)
        }

        /*
            Record a window of an incoming reference field.
            The input reference is the borrowing source. The field and the range
            stay on the projection the returned loan compares, because that field
            was not captured as its own descriptor.
        */
        fn incoming_reference_window(
            &mut self,
            actual: Handle<FlowOperandFact>,
            path: &[PlaceSegment],
            suffix: &[PlaceSegment],
            destination: TypeReferenceHandle,
            at: Handle<FlowStorageSnapshot>,
        ) -> Option<Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>> {
            if suffix.is_empty() || suffix.len() > path.len() {
                return None;
            }
            let source_path = &path[..path.len() - suffix.len()];
            let actual_value = *self.context.control.operands.get(actual);
            let selected = project_type_reference_from_segments(
                self.program,
                actual_value.type_reference,
                source_path,
            )?;
            let selected = reference_shell(self.program, selected)?;
            let desired = reference_shell(self.program, destination)?;
            if !(selected.access == desired.access
                || selected.access == language_semantics::ReferenceAccess::Mutable)
            {
                return None;
            }
            let actual_element = collection_element_type_reference(self.program, selected.referee)?;
            let expected_element =
                collection_element_type_reference(self.program, desired.referee)?;
            if !storage_origin_types_match(self.program, actual_element, expected_element) {
                return None;
            }
            let roots = super::super::operand_values::captured_lifetime_roots(
                self.program,
                &self.context.control,
                &actual_value,
            )?;
            let retention = self.append_retention(&roots);
            let reborrow_sources = self.context.control.operand_reborrow_sources.insert_many([
                checked_trees::FlowOperandReborrowSource {
                    operand: actual,
                    source_path: Default::default(),
                    projection: self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(path.iter().copied()),
                },
            ]);
            if self.context.control.operand_retentions.is_valid(retention) {
                self.context
                    .control
                    .operand_retentions
                    .get_mut(retention)
                    .reborrow_sources = reborrow_sources;
            }
            let value = self.context.control.operands.insert(FlowOperandFact {
                type_reference: destination,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                storage_snapshot: at,
                retention,
                ..Default::default()
            });
            Some(vec![(value, Vec::new())])
        }

        fn withhold_projected_pointer(
            &mut self,
            source: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
        ) -> Handle<FlowOperandFact> {
            if projection.is_empty() {
                return source;
            }
            let mut value = *self.context.control.operands.get(source);
            value.referents = Default::default();
            value.value_source = source;
            self.context.control.operands.insert(value)
        }
    }
}
mod dispatch {

    /*
        Match effects fork after each comparison and meet at the result join.
        A saved subject is not a promise about its storage after a pattern writes it;
        only unchanged stable inputs can publish branch-local comparison evidence.

        Completed scalars, predicate-only domains and positive IEEE observations join
        with the result. Scalars must agree when each arm finishes, before a later
        pattern or sibling can mutate their former storage; unknown arms publish none.
        Each covered arm
        contributes the predicates already carried by the value it yields, and
        the match result keeps only the consequences every arm carries. A fact
        about some other place in the surrounding context does not move onto the
        result. Owned qualifications first rebase the exact yielded occurrence;
        only identical membership and authority present in every covered arm survive.
        This publication transports authorized provenance, never establishes it.
    */
    use crate::flow::expression::Execution;
    mod owned_cases {
        //! A nested case decision may constrain only its actual unchanged owned home.

        use super::Execution;
        use arena::{Handle, HandleSpan};
        use checked_trees::{FlowOperandAddressProjection, FlowOperandCompletion, FlowOperandFact};
        use std::collections::BTreeSet;
        use typed_trees::expression::{ExpressionHandle, TableMatchArm};
        use validation::completed_storage::{StoredObservation, contents_preserved_from};

        pub(super) struct OwnedCaseSubject {
            place: validation::CanonicalPlace,
            captured_storage: Handle<checked_trees::FlowStorageSnapshot>,
            checked_storage: Handle<checked_trees::FlowStorageSnapshot>,
            source_current: bool,
            path: Option<HandleSpan<facts::PlaceSegment>>,
        }

        impl Execution<'_, '_, '_> {
            pub(super) fn owned_case_subject(
                &self,
                subject: &FlowOperandFact,
            ) -> Option<OwnedCaseSubject> {
                if subject.state_symbol != self.state.symbol
                    || subject.statement_index != self.statement_index
                    || subject.evaluation != self.evaluation
                    || subject.evaluation == checked_trees::FlowPermissionEvaluation::Unknown
                {
                    return None;
                }
                let mut current = *subject;
                let mut projections = Vec::new();
                let mut visited = BTreeSet::new();
                loop {
                    if current.state_symbol != self.state.symbol
                        || !current.type_reference.is_valid()
                        || validation::reference_shell(self.program, current.type_reference)
                            .is_some()
                        || self.program.type_multiplicity(current.type_reference)
                            == language_semantics::Multiplicity::Unrestricted
                        || matches!(current.completion, FlowOperandCompletion::Copied { .. })
                    {
                        return None;
                    }
                    let Some(projection) = current.address_projection else {
                        if current.backing.is_valid() {
                            return None;
                        }
                        break;
                    };
                    let FlowOperandAddressProjection::Place(path) = projection else {
                        return None;
                    };
                    let backing = current.backing;
                    if !self.context.control.operands.is_valid(backing)
                        || !visited.insert((backing.arena_index(), backing.generation()))
                    {
                        return None;
                    }
                    let segments = self.context.control.operand_child_paths.span(path)?;
                    let receiver = self.context.control.operands.get(backing);
                    let projected = validation::owned_projection_type_reference(
                        self.program,
                        receiver.type_reference,
                        segments,
                    )?;
                    if self.program.normalized_type_identity(projected)
                        != self
                            .program
                            .normalized_type_identity(current.type_reference)
                    {
                        return None;
                    }
                    projections.push(path);
                    current = *self.context.control.operands.get(backing);
                }
                let mut place = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    current.statement_index,
                    current.expression,
                )?;
                // An expression namespace needs its own completed value formation.
                // In particular, a missing saved supplier is not a new owner.
                if place.root.as_expression().is_some()
                    && !matches!(
                        current.completion,
                        FlowOperandCompletion::Constructed(_)
                            | FlowOperandCompletion::InvocationResult { .. }
                    )
                {
                    return None;
                }
                for path in projections.into_iter().rev() {
                    place
                        .segments
                        .extend_from_slice(self.context.control.operand_child_paths.span(path)?);
                }
                if !place.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Field { .. }
                            | facts::PlaceSegment::Case { .. }
                            | facts::PlaceSegment::FixedIndex { .. }
                    )
                }) {
                    // A live selector is not the saved index value. Without a fixed
                    // captured coordinate it cannot select an owner's cleanup path.
                    return None;
                }
                let reference = validation::canonical_place_type_reference(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    &place,
                )?;
                if self.program.normalized_type_identity(reference)
                    != self
                        .program
                        .normalized_type_identity(subject.type_reference)
                    || !self
                        .context
                        .control
                        .storage_snapshots
                        .is_valid(current.storage_snapshot)
                    || !self
                        .context
                        .control
                        .storage_snapshots
                        .is_valid(self.storage_snapshot)
                    || !contents_preserved_from(
                        self.program,
                        &self.context.control,
                        self.storage_snapshot,
                        &place,
                        &[current.storage_snapshot],
                        StoredObservation::InlineContents,
                    )
                {
                    return None;
                }
                Some(OwnedCaseSubject {
                    place,
                    captured_storage: current.storage_snapshot,
                    checked_storage: self.storage_snapshot,
                    source_current: true,
                    path: None,
                })
            }

            pub(super) fn retain_owned_case_selection(
                &mut self,
                expression: ExpressionHandle,
                source_arm: Handle<TableMatchArm>,
                subject: &FlowOperandFact,
                owner: &mut OwnedCaseSubject,
                variant: symbols::SymbolHandle,
                miss_excludes_case: bool,
            ) {
                if !owner.source_current {
                    return;
                }
                if owner.checked_storage != self.storage_snapshot {
                    owner.source_current = self
                        .context
                        .control
                        .storage_snapshots
                        .is_valid(self.storage_snapshot)
                        && contents_preserved_from(
                            self.program,
                            &self.context.control,
                            self.storage_snapshot,
                            &owner.place,
                            &[owner.checked_storage],
                            StoredObservation::InlineContents,
                        );
                    if !owner.source_current {
                        return;
                    }
                    owner.checked_storage = self.storage_snapshot;
                }
                let subject = self.context.control.operands.insert(*subject);
                let path = *owner.path.get_or_insert_with(|| {
                    self.context
                        .control
                        .operand_referent_segments
                        .insert_many(owner.place.segments.iter().copied())
                });
                let rows = &mut self
                    .context
                    .control
                    .statements
                    .get_mut(self.statement_fact)
                    .owned_case_selections;
                self.context.control.owned_case_selections.append_to_span(
                    rows,
                    checked_trees::FlowOwnedCaseSelection {
                        machine_symbol: self.machine.symbol,
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                        statement: self.statement_fact,
                        expression,
                        source_arm,
                        evaluation: self.evaluation,
                        subject,
                        root: owner.place.root,
                        path,
                        variant,
                        miss_excludes_case,
                        captured_storage: owner.captured_storage,
                        selected_storage: self.storage_snapshot,
                    },
                );
            }
        }
    }
    use crate::flow::expression::value_observations::OwnedValueObservation;
    use crate::flow::state_values::qualifications::CarriedPredicate;
    use arena::HandleSpan;
    use checked_trees::expression::ExpressionHandle;
    use checked_trees::{
        CheckedOperatorOccurrence, FlowConstraintRef, FlowOperandAlternative,
        FlowSemanticContextRef,
    };
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint, ScalarValue};
    use typed_trees::expression::{MatchPattern, TableMatchExpression};
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;
    use validation::expression_result_type_reference;
    use validation::field_domains::domain_requires_provenance;
    use validation::peel_declared_constraints;
    use validation::place_has_builtin_coordinates;
    use validation::structural_match_pattern_case;
    use validation::structural_match_pattern_is_total;
    use validation::structural_match_subject_type;

    impl Execution<'_, '_, '_> {
        pub(super) fn dispatch(
            &mut self,
            expression: ExpressionHandle,
            dispatch: TableMatchExpression,
            destination: TypeReferenceHandle,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) -> HandleSpan<FlowOperandAlternative> {
            let destination = if destination.is_valid() {
                destination
            } else {
                self.context
                    .expression_result_type_at(self.program, self.machine, self.state, expression)
                    .unwrap_or_default()
            };
            let structural =
                structural_match_subject_type(self.program, self.machine, self.state, &dispatch);
            let subject_type = self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                dispatch.subject,
            );
            let mut subject = if structural.is_some()
                && subject_type.is_some_and(|reference| self.is_view_type(reference))
            {
                self.evaluated_place(dispatch.subject, contexts, constraints)
            } else {
                self.evaluated_operand_as(
                    dispatch.subject,
                    subject_type.unwrap_or_default(),
                    contexts,
                    constraints,
                )
            };
            let outer_captures = self.match_subjects.len();
            if dispatch.capture.is_valid() {
                let anchor = self
                    .program
                    .expression_table
                    .value_capture(dispatch.capture)
                    .value_anchor;
                // Patterns and their effects have not run. This exact saved value
                // retains its producer's complete owned fact roster, including
                // conditional field qualifications, before later source writes.
                self.publish_owned_capture(anchor, &mut subject, contexts, constraints);
                self.match_subjects
                    .push((dispatch.capture, subject.capture));
            }
            let subject_operand = subject.capture;
            let mut owned_subject = self.owned_case_subject(&subject_operand);
            let subject_writes = self.operand_writes.len();
            let mut joined = None;
            let mut completed_writes = Vec::new();
            let mut completed_reference_branches = Vec::new();
            let mut boolean_coverage = [false; 2];
            let mut case_coverage = Vec::new();
            let mut shared_predicates: Option<Vec<CarriedPredicate>> = None;
            let mut shared_scalar: Option<Option<ScalarValue>> = None;
            let mut shared_members: Option<Vec<(Vec<facts::PlaceSegment>, OwnedValueObservation)>> =
                None;
            let mut shared_qualifications = None;
            let mut value_coverage = false;
            let mut alternatives = HandleSpan::empty();
            let mut prior_evaluation = arena::Handle::invalid();
            for (ordinal, arm) in self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .enumerate()
            {
                let mut evaluation_inputs = Vec::new();
                if !prior_evaluation.is_valid() {
                    evaluation_inputs.push(subject_operand);
                }
                let mut covered = matches!(arm.pattern, MatchPattern::Wildcard);
                let selected_case = structural.and_then(|_| {
                    structural_match_pattern_case(
                        self.program,
                        self.machine,
                        self.state,
                        &dispatch,
                        arm.pattern,
                    )
                });
                if let Some(reference) = structural
                    && let Some(case) = selected_case
                    && structural_match_pattern_is_total(self.program, arm.pattern)
                {
                    crate::shared::collections::push_unique(&mut case_coverage, case);
                    covered |= validation::structural_match_cases_are_covered(
                        self.program,
                        reference,
                        expression,
                        &case_coverage,
                    );
                }
                if let MatchPattern::Value(pattern) = arm.pattern {
                    let pattern_operand = self
                        .evaluated_operand(pattern, contexts, constraints)
                        .capture;
                    evaluation_inputs.push(pattern_operand);
                    if let Ok(ordinal) = u32::try_from(ordinal) {
                        let source_arm = dispatch.arms.handle_at(ordinal);
                        self.record_operator_invocation(
                            expression,
                            CheckedOperatorOccurrence::MatchEquality { source_arm },
                            &[subject_operand, pattern_operand],
                            *constraints,
                        );
                    }
                    if let Some(value) = self.program.expression_table.boolean_node(pattern) {
                        boolean_coverage[usize::from(*value)] = true;
                        covered = boolean_coverage.iter().all(|value| *value);
                    }
                }
                let mut selected_contexts = *contexts;
                let mut selected_constraints = *constraints;
                if let MatchPattern::Destructure(pattern) = arm.pattern {
                    evaluation_inputs.push(
                        self.evaluated_operand(pattern.condition, contexts, constraints)
                            .capture,
                    );
                    selected_contexts = *contexts;
                    selected_constraints = *constraints;
                    let prefix = self.write_prefix();
                    for (matched, branch_contexts, branch_constraints) in [
                        (true, &mut selected_contexts, &mut selected_constraints),
                        (false, &mut *contexts, &mut *constraints),
                    ] {
                        super::super::exits::append_predicate_context(
                            self.program,
                            self.semantic,
                            self.context,
                            self.state.symbol,
                            self.statement_index,
                            crate::flow::GuardObservation::evaluated(
                                pattern.condition,
                                matched,
                                self.statement_index,
                            ),
                            ProgramPoint::Statement {
                                machine_symbol: self.machine.symbol,
                                state_symbol: self.state.symbol,
                                statement_index: self.statement_index,
                            },
                            prefix.writes,
                            prefix.unknown,
                            branch_contexts,
                            branch_constraints,
                        );
                    }
                }
                let branch_prefix = self.write_prefix();
                if self
                    .changed_operand_sources(&[(dispatch.subject, subject_writes)])
                    .is_empty()
                    && let Ok(ordinal) = u32::try_from(ordinal)
                {
                    let source_arm = dispatch.arms.handle_at(ordinal);
                    let prefix = self.write_prefix();
                    let point = ProgramPoint::Statement {
                        machine_symbol: self.machine.symbol,
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                    };
                    for (matched, branch_contexts, branch_constraints) in [
                        (true, &mut selected_contexts, &mut selected_constraints),
                        (false, &mut *contexts, &mut *constraints),
                    ] {
                        super::super::exits::append_match_pattern_context(
                            self.program,
                            self.semantic,
                            self.context,
                            self.state.symbol,
                            self.statement_index,
                            FactPayload::MatchPattern {
                                expression,
                                arm: source_arm,
                                matched,
                            },
                            point,
                            prefix.writes,
                            prefix.unknown,
                            branch_contexts,
                            branch_constraints,
                        );
                    }
                }
                if let Some(owned_subject) = &mut owned_subject
                    && let Some(case) = selected_case
                    && let Ok(ordinal) = u32::try_from(ordinal)
                {
                    self.retain_owned_case_selection(
                        expression,
                        dispatch.arms.handle_at(ordinal),
                        &subject_operand,
                        owned_subject,
                        case,
                        structural_match_pattern_is_total(self.program, arm.pattern),
                    );
                }
                let completed = self.evaluated_operand_as(
                    arm.value,
                    destination,
                    &mut selected_contexts,
                    &mut selected_constraints,
                );
                let mut value_contexts = selected_contexts;
                let mut value_constraints = selected_constraints;
                self.append_completed_qualifications(
                    expression,
                    &[],
                    destination,
                    &completed,
                    &mut value_contexts,
                    &mut value_constraints,
                );
                let qualifications = self.value_memberships(expression, value_contexts);
                if let Some(shared) = &mut shared_qualifications {
                    super::live_memberships::retain_common(shared, &qualifications);
                } else {
                    shared_qualifications = Some(qualifications);
                }
                let members = self.captured_owned_value_observations(&[], destination, &completed);
                if let Some(shared) = &mut shared_members {
                    shared.retain_mut(|(path, observation)| {
                        let common = members
                            .iter()
                            .find(|(incoming_path, _)| incoming_path == path)
                            .and_then(|(_, incoming)| observation.common_consequence(incoming));
                        if let Some(common) = common {
                            *observation = common;
                            true
                        } else {
                            false
                        }
                    });
                } else {
                    shared_members = Some(members);
                }
                let scalar = self.scalar_value_on_value(arm.value, &selected_contexts);
                shared_scalar = Some(match shared_scalar {
                    None => scalar,
                    Some(previous) => previous.filter(|previous| scalar.as_ref() == Some(previous)),
                });
                let value = self.context.control.operands.append(completed.capture);
                let source_arm = u32::try_from(ordinal)
                    .map(|ordinal| dispatch.arms.handle_at(ordinal))
                    .unwrap_or_default();
                let evaluation_inputs =
                    self.context.control.operands.insert_many(evaluation_inputs);
                prior_evaluation = self.context.control.operand_alternatives.append_to_span(
                    &mut alternatives,
                    FlowOperandAlternative {
                        prior_evaluation,
                        evaluation_inputs,
                        source_arm,
                        value,
                    },
                );
                let yielded = self.predicates_on_value(arm.value, &selected_contexts);
                match shared_predicates.as_mut() {
                    Some(shared) => shared.retain_mut(|predicate| {
                        let mut common = None;
                        for incoming in &yielded {
                            if let Some(candidate) = predicate.common_consequence(*incoming)
                                && common.is_none_or(|previous| candidate.implies(previous))
                            {
                                common = Some(candidate);
                            }
                        }
                        if let Some(common) = common {
                            *predicate = common;
                            true
                        } else {
                            false
                        }
                    }),
                    None => shared_predicates = Some(yielded),
                }
                completed_writes
                    .extend(self.operand_writes[branch_prefix.length..].iter().cloned());
                completed_reference_branches.push(self.write_prefix());
                self.restore_write_prefix(branch_prefix);
                if let Some((previous_contexts, previous_constraints)) = joined {
                    self.meet(
                        previous_contexts,
                        previous_constraints,
                        &mut selected_contexts,
                        &mut selected_constraints,
                    );
                }
                joined = Some((selected_contexts, selected_constraints));
                if covered {
                    value_coverage = true;
                    break;
                }
            }
            if !value_coverage {
                completed_reference_branches.push(self.write_prefix());
            }
            for writes in completed_writes {
                self.record_operand_writes(writes);
            }
            self.join_storage_prefixes(&completed_reference_branches);
            self.match_subjects.truncate(outer_captures);
            if let Some((joined_contexts, joined_constraints)) = joined {
                *contexts = joined_contexts;
                *constraints = joined_constraints;
            }
            let subject = self.context.control.operands.insert(subject_operand);
            self.retain_value_observation(
                expression,
                subject,
                subject_operand.referents,
                *contexts,
                *constraints,
            );
            if value_coverage {
                if let Some(qualifications) = shared_qualifications {
                    self.publish_live_memberships(qualifications, contexts, constraints);
                }
                if let Some(members) = shared_members {
                    self.publish_owned_value_observations(
                        expression,
                        members,
                        contexts,
                        constraints,
                    );
                }
                if let Some(shared) = shared_predicates {
                    self.publish_joined_values(
                        expression,
                        &shared,
                        shared_scalar.flatten(),
                        contexts,
                        constraints,
                    );
                }
            }
            if value_coverage {
                alternatives
            } else {
                HandleSpan::empty()
            }
        }

        /*
            Read this completed arm's owned scalar, preserving the selected source
            meaning. A reference arm carries a descriptor, not its referent's scalar;
            an authored indexing operation likewise needs its own result snapshot.
        */
        fn scalar_value_on_value(
            &self,
            value: ExpressionHandle,
            contexts: &HandleSpan<FlowSemanticContextRef>,
        ) -> Option<ScalarValue> {
            if !crate::values::is_assignment_literal(self.program, value) {
                let reference = expression_result_type_reference(
                    self.program,
                    self.machine,
                    self.state,
                    value,
                )?;
                let reference = peel_declared_constraints(self.program, reference);
                if matches!(
                    self.program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Reference { .. }
                ) || self.program.primitive_type_reference(reference).is_none()
                {
                    return None;
                }
            }
            let source = self.completed_value_place(value)?;
            if matches!(source.root, PlaceRoot::Symbol(_))
                && !place_has_builtin_coordinates(
                    self.program,
                    self.machine,
                    Some(self.state),
                    value,
                )
            {
                return None;
            }
            crate::values::scalar_value_at_place(
                self.program,
                self.semantic,
                self.context
                    .semantic_context_views(self.semantic, *contexts),
                &source,
            )
        }

        /*
            Transportable predicates whose place is this arm value.

            Membership of a neighboring place stays where it is. A routed domain
            is absent here because copying it would drop the provenance the
            checker still has to rejoin.
        */
        fn predicates_on_value(
            &self,
            value: ExpressionHandle,
            contexts: &HandleSpan<FlowSemanticContextRef>,
        ) -> Vec<CarriedPredicate> {
            let Some(source) = self.completed_value_place(value) else {
                return Vec::new();
            };
            let mut predicates = Vec::new();
            for fact in crate::flow::active_context_facts(self.context, self.semantic, *contexts) {
                let predicate = if let FactPayload::FloatPredicate { predicate } = fact.payload {
                    CarriedPredicate::Float(predicate)
                } else if let Some((_, _, domain_symbol, semantic_domain)) =
                    fact.payload.domain_membership()
                {
                    if domain_requires_provenance(self.program, domain_symbol) {
                        continue;
                    }
                    let Some(domain) = self.program.domain_definition(domain_symbol) else {
                        continue;
                    };
                    if !domain.predicate_presence.is_present() || domain.alias.is_some() {
                        continue;
                    }
                    CarriedPredicate::Domain {
                        domain: HandleSpan::empty(),
                        domain_symbol,
                        semantic_domain,
                    }
                } else {
                    continue;
                };
                let Some(fact_place) = fact.place.as_place() else {
                    continue;
                };
                let Some(fact_place) = crate::flow::canonical_place_from_semantic_place(
                    self.program,
                    self.semantic,
                    self.semantic.places.get(fact_place),
                ) else {
                    continue;
                };
                if !crate::flow::canonical_places_agree(self.program, &source, &fact_place) {
                    continue;
                }
                crate::shared::collections::push_unique(&mut predicates, predicate);
            }
            predicates
        }

        fn publish_joined_values(
            &mut self,
            expression: ExpressionHandle,
            domains: &[CarriedPredicate],
            scalar: Option<ScalarValue>,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            if domains.is_empty() && scalar.is_none() {
                return;
            }
            let place = self.semantic.append_expression_place(expression);
            let point = ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            };
            let mut refs = HandleSpan::empty();
            if let Some(scalar) = scalar {
                let value = self.semantic.scalar_values.append(scalar);
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::propagated_evidence(self.state.symbol),
                    payload: FactPayload::AssignedScalarValue { value },
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            for domain in domains {
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::propagated_evidence(self.state.symbol),
                    payload: domain.payload(expression),
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            let context = self.semantic.append_context(point, refs);
            self.context
                .append_semantic_context(contexts, constraints, context);
        }
    }
}
mod dispatch_copies {

    /*
        Complete selected payload copies before target operands. Saved receiver
        observations authenticate the source address; source expressions identify
        occurrences but are never evaluated again. Reference leaves save descriptors
        at this boundary and retain ordinary live-memory semantics after it.
    */
    use super::{Execution, ReadPurpose};
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use crate::published::semantic::places::append_place;
    use arena::HandleSpan;
    use checked_trees::{
        FlowConstraintKind, FlowConstraintRef, FlowOperandCompletion, FlowOperandFact,
        FlowSemanticContextRef,
    };
    use facts::PlaceSegment;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::{TableTransition, TransitionTargetHandle, TransitionTargetNode};

    impl Execution<'_, '_, '_> {
        /*
            Read the value named by this occurrence. Address formation deliberately
            keeps the original source place; saved reference descriptors resolve to
            their actual referent without freezing that referent's current contents.
        */
        pub(super) fn completed_read_place(
            &self,
            expression: ExpressionHandle,
        ) -> Option<validation::CanonicalPlace> {
            if self.read_purpose == ReadPurpose::Address {
                return place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                );
            }
            self.completed_value_place(expression)
        }

        pub(super) fn completed_value_place(
            &self,
            expression: ExpressionHandle,
        ) -> Option<validation::CanonicalPlace> {
            let place = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            )?;
            if self.context.active_completed_copies.is_empty() {
                return Some(place);
            }
            self.context.completed_read_place(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
                &place,
            )
        }

        pub(super) fn capture_dispatch_payloads(
            &mut self,
            transition: &TableTransition,
            target: TransitionTargetHandle,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) -> HandleSpan<FlowOperandFact> {
            if transition.target != target {
                return HandleSpan::empty();
            }
            let statements = super::super::reference_spans::appended_span_since(
                &self.context.control.statements,
                self.context.state_statements_start,
            );
            let observations = self
                .context
                .control
                .value_observations
                .span_or_empty(
                    self.context
                        .control
                        .statements
                        .get(self.statement_fact)
                        .value_observations,
                )
                .to_vec();
            let mut extraction = validation::DispatchExtraction::of_transition_arm(
                self.program,
                self.machine,
                self.state,
                self.statement_index,
            );
            let mut subjects = Vec::new();
            for observation in observations {
                if let Some(subject) = super::super::observations::captured_case_subject_in_state(
                    self.program,
                    self.semantic,
                    &self.context.control,
                    &self.context.contexts,
                    self.borrow,
                    self.machine,
                    self.state,
                    statements,
                    self.statement_index,
                    observation.expression,
                ) {
                    let source = self.context.control.operands.get(subject.source);
                    extraction.rejoin_completed_subject(subject.observed, source.expression);
                    subjects.push(subject);
                }
            }
            if subjects.is_empty() {
                return HandleSpan::empty();
            }
            let target_expressions = match self.program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => self
                    .program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
                TransitionTargetNode::Value(expression) => vec![*expression],
                TransitionTargetNode::Terminal => Vec::new(),
            };
            let target_symbol = self
                .invocations
                .iter()
                .find(|invocation| invocation.site == super::InvocationSite::Transition(target))
                .map_or(symbols::SymbolHandle::invalid(), |invocation| {
                    invocation.call.target_symbol
                });
            let value_reads = crate::borrow::accesses::dispatch_target_value_reads(
                self.program,
                &target_expressions,
                self.state.symbol,
                self.statement_index,
                self.machine.symbol,
                target_symbol,
                extraction.clone(),
            );
            let mut payloads = Vec::new();
            for expression in value_reads {
                if let Some(payload) =
                    extraction.extracted_copy_payload(self.program, expression, |payload| {
                        validation::expression_type_reference_in_state(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            payload,
                        )
                        .is_some_and(|reference| {
                            validation::type_satisfies_declared_property(
                                self.program,
                                &[],
                                reference,
                                validation::DeclaredPropertyRequirement::Copy,
                            ) && validation::has_contents(
                                self.program,
                                reference,
                                &[],
                                validation::ContentsRequirement::StoredValue,
                            )
                        })
                    })
                    && !payloads.iter().any(|previous| {
                        expressions(self.program).expressions_structurally_equal(*previous, payload)
                    })
                {
                    payloads.push(payload);
                }
            }
            let mut copies = Vec::new();
            for payload in payloads {
                let Some(member) = self.program.expression_table.member_node(payload) else {
                    continue;
                };
                let mut matching = subjects.iter().filter(|subject| {
                    let source = self.context.control.operands.get(subject.source);
                    expressions(self.program)
                        .expressions_structurally_equal(source.expression, member.receiver)
                });
                let Some(subject) = matching.next() else {
                    continue;
                };
                if matching.next().is_some() {
                    continue;
                }
                let source = *self.context.control.operands.get(subject.source);
                let receiver = *self.context.control.operands.get(subject.receiver);
                let Some(reference) = validation::expression_type_reference_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    payload,
                ) else {
                    continue;
                };
                let entry = FlowOperandFact {
                    expression: payload,
                    state_symbol: self.state.symbol,
                    statement_index: self.statement_index,
                    evaluation: self.evaluation,
                    type_reference: reference,
                    storage_snapshot: self.storage_snapshot,
                    entry_origin_writes: self.entry_origin_writes,
                    entry_origin_unknown_write: self.entry_origin_unknown_write,
                    entry_binding_writes: self.entry_binding_writes,
                    entry_binding_unknown_write: self.entry_binding_unknown_write,
                    constraints: *constraints,
                    backing: subject.receiver,
                    ..Default::default()
                };
                let retained_loan = subject.loans.iter().any(|loan| {
                    self.context
                        .contexts
                        .constraint_refs
                        .span_or_empty(*constraints)
                        .iter()
                        .any(|reference| {
                            reference.kind == (FlowConstraintKind::BorrowLoan { loan: *loan })
                        })
                });
                if !retained_loan
                    || !self.copy_source_coordinates_agree(statements, &source, &entry)
                {
                    continue;
                }
                let symbol =
                    validation::effective_member_symbol(self.program, member.receiver, member);
                let Some(variant) = facts::payload_variant_for_field(self.program, symbol) else {
                    continue;
                };
                let payload_places = self
                    .captured_reference_places(receiver.referents)
                    .into_iter()
                    .map(|place| {
                        place.projected(&[
                            PlaceSegment::Case { variant },
                            PlaceSegment::Field { symbol },
                        ])
                    })
                    .collect::<Option<Vec<_>>>()
                    .unwrap_or_default();
                let source_places = self.append_captured_references(&payload_places);
                let source_access =
                    self.record_case_access_places(payload, *constraints, source_places);
                self.publish_copy_contents(payload, contexts, constraints);
                let mut captured = entry;
                captured.case_access = source_access;
                captured.constraints = *constraints;
                captured.retention = self.capture_value_retention(&captured);
                let source_handle = self.context.control.operands.append(captured);
                let references =
                    self.capture_owned_reference_leaves(source_handle, &payload_places);
                let copy =
                    self.context
                        .control
                        .copied_values
                        .append(checked_trees::FlowCopiedValue {
                            source: source_handle,
                            references,
                            uses: HandleSpan::empty(),
                        });
                captured.completion = FlowOperandCompletion::Copied {
                    copy,
                    path: HandleSpan::empty(),
                };
                copies.push(captured);
            }
            self.context.control.operands.insert_many(copies)
        }

        fn publish_copy_contents(
            &mut self,
            payload: ExpressionHandle,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let Some(mut source) = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                payload,
            ) else {
                return;
            };
            let active = super::super::active_context_handles(&self.context.contexts, *contexts);
            super::super::narrow_pinned_index_segments(
                self.program,
                self.semantic,
                self.state.symbol,
                self.statement_index,
                &active,
                &mut source.segments,
            );
            let source = append_place(self.semantic, source.root, &source.segments);
            let destination =
                append_place(self.semantic, facts::PlaceRoot::Expression(payload), &[]);
            let point = facts::ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            };
            let mut references = HandleSpan::empty();
            super::super::transfers::append_completed_copy_predicates(
                self.program,
                self.semantic,
                self.context,
                *contexts,
                source,
                destination,
                point,
                &mut references,
            );
            super::super::transfers::publish_statement_contexts(
                self.semantic,
                self.context,
                self.machine.symbol,
                self.state.symbol,
                self.statement_index,
                references,
                contexts,
                constraints,
            );
        }

        fn copy_source_coordinates_agree(
            &self,
            statements: HandleSpan<checked_trees::FlowStatementFact>,
            source: &FlowOperandFact,
            entry: &FlowOperandFact,
        ) -> bool {
            let Some(coordinate) = place_at(
                self.program,
                self.state.symbol,
                source.statement_index,
                source.expression,
            ) else {
                return false;
            };
            let agrees = |coordinate: &validation::CanonicalPlace| {
                super::super::operand_contexts::operand_bindings_agree_in_state(
                    self.program,
                    &self.context.control,
                    statements,
                    source,
                    entry,
                    coordinate,
                )
            };
            agrees(&coordinate)
                && coordinate.segments.iter().all(|segment| {
                    let PlaceSegment::Index { expression } = segment else {
                        return true;
                    };
                    place_at(
                        self.program,
                        self.state.symbol,
                        source.statement_index,
                        *expression,
                    )
                    .is_some_and(|selector| agrees(&selector))
                })
        }

        pub(super) fn retain_copy_use(
            &mut self,
            expression: ExpressionHandle,
            receiver: &FlowOperandFact,
        ) {
            let mut current = *receiver;
            let mut visited = Vec::new();
            loop {
                if let FlowOperandCompletion::Copied { copy, .. } = current.completion {
                    let record = *self.context.control.copied_values.get(copy);
                    let usage = checked_trees::FlowCopiedValueUse {
                        expression,
                        evaluation: self.evaluation,
                    };
                    if !self
                        .context
                        .control
                        .copied_value_uses
                        .span_or_empty(record.uses)
                        .contains(&usage)
                    {
                        let mut uses = record.uses;
                        self.context
                            .control
                            .copied_value_uses
                            .append_to_span(&mut uses, usage);
                        self.context.control.copied_values.get_mut(copy).uses = uses;
                    }
                    return;
                }
                if !self.context.control.operands.is_valid(current.backing)
                    || visited.contains(&current.backing)
                {
                    return;
                }
                visited.push(current.backing);
                current = *self.context.control.operands.get(current.backing);
            }
        }

        /*
            A by-value read projects one completed copy. Address formation deliberately
            keeps the ordinary source lane. Crossing a copied reference ends this
            owned-value projection and resumes live-memory observation.
        */
        pub(super) fn dispatch_copy_value(
            &mut self,
            expression: ExpressionHandle,
            constraints: HandleSpan<FlowConstraintRef>,
        ) -> Option<FlowOperandFact> {
            if self.read_purpose != ReadPurpose::Value {
                return None;
            }
            let place = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            )?;
            let copies = self
                .context
                .active_completed_copies
                .handles()
                .map(|handle| (handle, *self.context.control.operands.get(handle)))
                .collect::<Vec<_>>();
            let mut matching = Vec::new();
            for (captured_handle, captured) in copies {
                if !validation::DispatchExtraction::reads_payload_binding(
                    self.program,
                    expression,
                    captured.expression,
                ) {
                    continue;
                }
                let prefix = place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    captured.expression,
                )?;
                if prefix.root != place.root
                    || prefix.segments.len() > place.segments.len()
                    || !prefix
                        .segments
                        .iter()
                        .zip(&place.segments)
                        .all(|(left, right)| {
                            super::super::canonical_place_segments_equal(
                                self.program,
                                *left,
                                *right,
                            )
                        })
                {
                    continue;
                }
                let mut path = place.segments[prefix.segments.len()..].to_vec();
                let active = self
                    .context
                    .contexts
                    .constraint_refs
                    .span_or_empty(constraints)
                    .iter()
                    .filter_map(|reference| match reference.kind {
                        FlowConstraintKind::SemanticContext { context } => Some(context),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                super::super::narrow_pinned_index_segments(
                    self.program,
                    self.semantic,
                    self.state.symbol,
                    self.statement_index,
                    &active,
                    &mut path,
                );
                let path = path.as_slice();
                let Some(reference) = validation::owned_projection_type_reference(
                    self.program,
                    captured.type_reference,
                    path,
                ) else {
                    continue;
                };
                let FlowOperandCompletion::Copied { copy, .. } = captured.completion else {
                    continue;
                };
                let mut projected = captured;
                projected.expression = expression;
                projected.backing = captured_handle;
                projected.type_reference = reference;
                projected.constraints = constraints;
                projected.entry_origin_writes = self.entry_origin_writes;
                projected.entry_origin_unknown_write = self.entry_origin_unknown_write;
                projected.entry_binding_writes = self.entry_binding_writes;
                projected.entry_binding_unknown_write = self.entry_binding_unknown_write;
                if self.is_view_type(reference) {
                    let copy = self.context.control.copied_values.get(copy);
                    let mut leaves = self
                        .context
                        .control
                        .captured_reference_leaves
                        .span_or_empty(copy.references)
                        .iter()
                        .filter(|leaf| {
                            self.context
                                .control
                                .operand_child_paths
                                .span_or_empty(leaf.path)
                                == path
                        });
                    projected.referents = HandleSpan::empty();
                    projected.retention = arena::Handle::invalid();
                    if let Some(leaf) = leaves.next()
                        && leaves.next().is_none()
                    {
                        projected.referents = leaf.referents;
                        projected.retention = leaf.retention;
                    }
                } else {
                    projected.referents =
                        self.append_referent_places(&[validation::CanonicalPlace {
                            root: facts::PlaceRoot::Expression(captured.expression),
                            segments: path.to_vec(),
                        }]);
                    projected.retention = self.captured_retention_at_path(captured, path);
                }
                let path = self
                    .context
                    .control
                    .operand_child_paths
                    .insert_many(path.iter().copied());
                projected.completion = FlowOperandCompletion::Copied { copy, path };
                matching.push(projected);
            }
            (matching.len() == 1).then(|| matching[0])
        }
    }
}
mod live_memberships {
    /*
        Meet live storage memberships by their semantic subject and evidence.

        Distinct calls can hand back the same field promise in distinct fact records.
        Context-handle intersection alone loses that promise. Every incoming branch
        must carry it on the exact place and domain instance with the same evidence;
        this transports qualification, never establishes content or equates lineage.
    */
    use super::Execution;
    use arena::HandleSpan;
    use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint, QualificationEvidence};

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct LiveMembership {
        place: validation::CanonicalPlace,
        domain: symbols::SymbolHandle,
        instance: language_semantics::SemanticDomainId,
        evidence: QualificationEvidence,
    }

    /*
        Compute each structural path key once per batch. A bucket is only a
        candidate set: full membership equality below still checks root/path,
        domain instance and qualification evidence, including on hash collisions.
    */
    fn membership_buckets(memberships: &[LiveMembership]) -> facts::places::PlaceBucketIndex {
        facts::places::PlaceBucketIndex::new(memberships.iter().map(|membership| {
            facts::places::place_bucket_hash(membership.place.root, &membership.place.segments)
        }))
    }

    fn deduplicate(memberships: &mut Vec<LiveMembership>) {
        let buckets = membership_buckets(memberships);
        let first =
            buckets.group_equal_positions(|left, right| memberships[left] == memberships[right]);
        let mut position = 0;
        memberships.retain(|_| {
            let retained = first[position] == position;
            position += 1;
            retained
        });
    }

    pub(super) fn retain_common(
        memberships: &mut Vec<LiveMembership>,
        incoming: &[LiveMembership],
    ) {
        let buckets = membership_buckets(incoming);
        memberships.retain_mut(|membership| {
            let key =
                facts::places::place_bucket_hash(membership.place.root, &membership.place.segments);
            let mut common: Option<QualificationEvidence> = None;
            for position in buckets.candidates(key) {
                let next = &incoming[position];
                if membership.place != next.place
                    || membership.domain != next.domain
                    || membership.instance != next.instance
                {
                    continue;
                }
                let Some(evidence) = membership.evidence.common_consequence(next.evidence) else {
                    continue;
                };
                common = match common {
                    Some(retained) => retained.available_alternative(evidence),
                    None => Some(evidence),
                };
            }
            let Some(evidence) = common else {
                return false;
            };
            membership.evidence = evidence;
            true
        });
    }

    impl Execution<'_, '_, '_> {
        pub(super) fn live_memberships(
            &self,
            contexts: HandleSpan<FlowSemanticContextRef>,
        ) -> Vec<LiveMembership> {
            let mut memberships = Vec::new();
            for fact in crate::flow::active_context_facts(self.context, self.semantic, contexts) {
                let Some((_, _, domain, instance)) = fact.payload.domain_membership() else {
                    continue;
                };
                let Some(place) = fact.place.as_place().and_then(|place| {
                    crate::flow::canonical_place_from_semantic_place(
                        self.program,
                        self.semantic,
                        self.semantic.places.get(place),
                    )
                }) else {
                    continue;
                };
                let membership = LiveMembership {
                    place,
                    domain,
                    instance,
                    evidence: fact.evidence,
                };
                memberships.push(membership);
            }
            deduplicate(&mut memberships);
            memberships
        }

        /*
            Keep only the selected value's owned root and members; unrelated live
            storage cannot supply a qualification for the selection result.
        */
        pub(super) fn value_memberships(
            &self,
            expression: typed_trees::expression::ExpressionHandle,
            contexts: HandleSpan<FlowSemanticContextRef>,
        ) -> Vec<LiveMembership> {
            self.live_memberships(contexts)
                .into_iter()
                .filter(|membership| {
                    membership.place.root == facts::PlaceRoot::Expression(expression)
                })
                .collect()
        }

        pub(super) fn publish_live_memberships(
            &mut self,
            memberships: Vec<LiveMembership>,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let retained = self.live_memberships(*contexts);
            let retained_buckets = membership_buckets(&retained);
            let point = ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            };
            for membership in memberships {
                let key = facts::places::place_bucket_hash(
                    membership.place.root,
                    &membership.place.segments,
                );
                if retained_buckets
                    .candidates(key)
                    .any(|position| retained[position] == membership)
                {
                    continue;
                }
                let place = crate::published::semantic::places::append_place(
                    self.semantic,
                    membership.place.root,
                    &membership.place.segments,
                );
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: membership.evidence,
                    payload: FactPayload::DomainMembership {
                        value: typed_trees::expression::ExpressionHandle::invalid(),
                        domain: HandleSpan::empty(),
                        domain_symbol: membership.domain,
                        semantic_domain: membership.instance,
                    },
                });
                let mut references = HandleSpan::empty();
                self.semantic.append_ref(&mut references, fact);
                let context = self.semantic.append_context(point, references);
                self.context
                    .append_semantic_context(contexts, constraints, context);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn membership(index: usize) -> LiveMembership {
            LiveMembership {
                place: validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(symbols::SymbolHandle::invalid()),
                    segments: vec![facts::PlaceSegment::FixedIndex { index }],
                },
                domain: symbols::SymbolHandle::invalid(),
                instance: language_semantics::SemanticDomainId::NULL,
                evidence: QualificationEvidence::default(),
            }
        }

        #[test]
        fn membership_batches_preserve_order_instances_and_evidence() {
            let originals: Vec<_> = (0..128).rev().map(membership).collect();
            let mut values = originals.clone();
            values.extend(originals.iter().rev().cloned());
            let mut changed = membership(64);
            changed.evidence.origin = language_semantics::QualificationEvidenceOrigin::Propagated;
            values.push(changed.clone());
            let mut expected = originals.clone();
            expected.push(changed);
            let mut different_instance = membership(64);
            different_instance.instance = language_semantics::SemanticDomainId(1);
            values.push(different_instance.clone());
            expected.push(different_instance);
            deduplicate(&mut values);
            assert_eq!(values, expected);
            retain_common(&mut values, &originals);
            assert_eq!(values, originals);
            let incoming: Vec<_> = originals.iter().step_by(2).cloned().collect();
            retain_common(&mut values, &incoming);
            assert_eq!(values, incoming);
            retain_common(&mut values, &[]);
            assert!(values.is_empty());
            deduplicate(&mut values);
            assert!(values.is_empty());
        }
    }
}
mod reference_projection {
    /*
        Substitute a formal storage route through one completed actual. Returning a
        reference and installing a reference slot use the same typed crossings. The
        destination mode stops at the final descriptor's home; value mode loads it.
    */
    use super::Execution;
    use arena::Handle;
    use checked_trees::{FlowOperandFact, FlowOperandRetention};
    use facts::PlaceSegment;

    #[derive(Clone, Copy)]
    pub(super) enum ReferenceProjection {
        Value,
        Destination,
    }

    #[derive(Debug, Clone, Copy)]
    enum ReferenceProjectionRefusal {
        UnsupportedCoordinate,
        NoCapturedReferenceCrossing,
        OwnedDescriptorDestination,
        MissingCapturedLeaf,
        EmptyActualAddress,
        UnknownProjectedType { position: usize },
        UnavailableProjection { position: usize },
        MissingStoredDescriptor { position: usize },
        EmptyStoredDescriptor { position: usize },
    }

    impl ReferenceProjectionRefusal {
        fn position(self) -> Option<usize> {
            match self {
                Self::UnknownProjectedType { position }
                | Self::UnavailableProjection { position }
                | Self::MissingStoredDescriptor { position }
                | Self::EmptyStoredDescriptor { position } => Some(position),
                _ => None,
            }
        }
    }

    pub(super) struct CapturedReferenceProjection {
        pub(super) places: Vec<super::super::CapturedReferencePlace>,
        pub(super) retention: Handle<FlowOperandRetention>,
    }

    impl Execution<'_, '_, '_> {
        pub(super) fn capture_reference_projection(
            &mut self,
            actual: &FlowOperandFact,
            segments: &[PlaceSegment],
            projection: ReferenceProjection,
        ) -> Option<CapturedReferenceProjection> {
            let result = self.try_capture_reference_projection(actual, segments, projection);
            if let Err(reason) = &result
                && ::validation::trace_flags::check_sites()
            {
                eprintln!(
                    "CHECKSITE reference-projection-refusal machine={:?} state={:?} statement={} expression={:?} type={:?} destination={} path_count={} path_prefix={:?} position={:?} reason={:?}",
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    actual.expression,
                    actual.type_reference,
                    matches!(projection, ReferenceProjection::Destination),
                    segments.len(),
                    &segments[..segments.len().min(32)],
                    reason.position(),
                    reason,
                );
            }
            result.ok()
        }

        fn try_capture_reference_projection(
            &mut self,
            actual: &FlowOperandFact,
            segments: &[PlaceSegment],
            projection: ReferenceProjection,
        ) -> Result<CapturedReferenceProjection, ReferenceProjectionRefusal> {
            let read_final = matches!(projection, ReferenceProjection::Value);
            if !segments.iter().all(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Field { .. }
                        | PlaceSegment::Case { .. }
                        | PlaceSegment::FixedIndex { .. }
                )
            }) {
                return Err(ReferenceProjectionRefusal::UnsupportedCoordinate);
            }
            let mut start = 0;
            let mut retention;
            let mut places = if self.is_view_type(actual.type_reference) {
                retention = self.captured_retention_at_path(*(actual), &[]);
                self.captured_reference_places(actual.referents)
            } else {
                let length = (1..=segments.len())
                    .find(|length| {
                        !matches!(segments[length - 1], PlaceSegment::Case { .. })
                            && validation::project_type_reference_from_segments(
                                self.program,
                                actual.type_reference,
                                &segments[..*length],
                            )
                            .is_some_and(|reference| self.is_view_type(reference))
                    })
                    .ok_or(ReferenceProjectionRefusal::NoCapturedReferenceCrossing)?;
                if !read_final && length == segments.len() {
                    return Err(ReferenceProjectionRefusal::OwnedDescriptorDestination);
                }
                start = length;
                retention = self.captured_retention_at_path(*(actual), &segments[..length]);
                let capture = self.context.control.operands.insert(*actual);
                let selected = validation::completed_storage::reference_captures_at_path(
                    self.program,
                    &self.context.control,
                    capture,
                    &segments[..length],
                    self.storage_snapshot,
                )
                .and_then(|stored| stored.captures())
                .ok_or(ReferenceProjectionRefusal::MissingCapturedLeaf)?;
                let mut places = Vec::new();
                for descriptor in selected {
                    let addresses =
                        super::super::operand_values::captured_reference_places_with_formation(
                            self.program,
                            &self.context.control,
                            descriptor.capture,
                            descriptor.reference,
                            &descriptor.projection,
                        )
                        .ok_or(ReferenceProjectionRefusal::MissingCapturedLeaf)?;
                    if addresses.is_empty() {
                        return Err(ReferenceProjectionRefusal::EmptyActualAddress);
                    }
                    for address in addresses {
                        crate::shared::collections::push_unique(&mut places, address);
                    }
                }
                places
            };
            if places.is_empty() {
                return Err(ReferenceProjectionRefusal::EmptyActualAddress);
            }
            for (position, segment) in segments.iter().enumerate().skip(start) {
                let projected = validation::project_type_reference_from_segments(
                    self.program,
                    actual.type_reference,
                    &segments[..=position],
                )
                .ok_or(ReferenceProjectionRefusal::UnknownProjectedType { position })?;
                let reads_descriptor = !matches!(segment, PlaceSegment::Case { .. })
                    && self.is_view_type(projected)
                    && (read_final || position + 1 != segments.len());
                let mut projected_places = Vec::new();
                let mut roots = Vec::new();
                let mut lifetime_known = true;
                for place in places {
                    let place = super::super::CapturedReferencePlace {
                        place: validation::completed_storage::project_captured_storage_place(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            place.place,
                            std::slice::from_ref(segment),
                        )
                        .ok_or(ReferenceProjectionRefusal::UnavailableProjection { position })?,
                        formation: place.formation,
                    };
                    if reads_descriptor {
                        let stored = self.stored_view_referents_at_place(&place.place).ok_or(
                            ReferenceProjectionRefusal::MissingStoredDescriptor { position },
                        )?;
                        let stored = self.captured_reference_places(stored);
                        if stored.is_empty() {
                            return Err(ReferenceProjectionRefusal::EmptyStoredDescriptor {
                                position,
                            });
                        }
                        let stored_retention = self
                            .stored_retention_at_place(&place.place)
                            .unwrap_or_default();
                        if let Some(retained) =
                            self.context.control.retained_roots(stored_retention)
                        {
                            for root in retained {
                                if !roots.contains(root) {
                                    roots.push(*root);
                                }
                            }
                        } else {
                            lifetime_known = false;
                        }
                        for origin in stored {
                            crate::shared::collections::push_unique(&mut projected_places, origin);
                        }
                    } else if !projected_places.contains(&place) {
                        projected_places.push(place);
                    }
                }
                if reads_descriptor {
                    retention = if lifetime_known {
                        self.append_retention(&roots)
                    } else {
                        Handle::invalid()
                    };
                }
                places = projected_places;
            }
            Ok(CapturedReferenceProjection { places, retention })
        }
    }
}
mod result_facts {
    /*
        A completed result keeps its checked scalar guarantees at evaluation.

        Reuse validation's destination-independent result query: it joins all Match
        arms by normalized type identity and does not copy operand refinements onto
        arithmetic results. Publish after child effects and branch meets, never into
        a callee's requires context or an unselected arm. The subject is this result
        occurrence, not storage read while producing it.

        Checked domain qualifications and enforced floating ranges describe the
        completed result, not its operands or a receiving annotation. Range
        predicates join the same captured occurrence as tags; invocation checking
        need not reconstruct a source call
        or skip its preconditions. This is transport, not establishment authority.
        The normalized instance travels with the declaration through every fact copy.
        Equality of exact instances is not a license to equate different open index
        expressions; those still require their selected normalization/equality proof.
        Normal-return integer bounds use the selected declaration's contract, not
        body evaluation or a later read of mutable actuals. Publishing them at the
        expression occurrence lets constructors and assignments consume one common
        snapshot; a later sibling write invalidates source storage, not this result.
    */
    use crate::flow::append_constraint_ref;
    use crate::flow::expression::Execution;
    use crate::flow::reference_spans;
    use arena::HandleSpan;
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint};

    impl Execution<'_, '_, '_> {
        fn cast_predicate_observations(
            &self,
            expression: ExpressionHandle,
            target: typed_trees::types::TypeReferenceHandle,
            contexts: HandleSpan<FlowSemanticContextRef>,
        ) -> Vec<(symbols::SymbolHandle, language_semantics::SemanticDomainId)> {
            let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
            else {
                return Vec::new();
            };
            if cast.form.is_recast() {
                return Vec::new();
            }
            let Some(source_type) = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                cast.value,
            ) else {
                return Vec::new();
            };
            if !validation::declared_carriers_match(self.program, source_type, target) {
                return Vec::new();
            }
            let Some(source) = self.completed_value_place(cast.value) else {
                return Vec::new();
            };
            let handles = crate::flow::active_context_handles(&self.context.contexts, contexts);
            if !crate::flow::place_cases_are_selected(
                self.program,
                self.semantic,
                &handles,
                self.machine.symbol,
                self.state.symbol,
                self.statement_index,
                &source,
            ) {
                return Vec::new();
            }
            crate::flow::active_context_facts(self.context, self.semantic, contexts)
                .into_iter()
                .filter_map(|fact| {
                    let (_, _, symbol, identity) = fact.payload.domain_membership()?;
                    let domain = self.program.domain_definition(symbol)?;
                    if !domain.predicate_presence.is_present()
                        || domain.alias.is_some()
                        || validation::field_domains::domain_requires_provenance(
                            self.program,
                            symbol,
                        )
                    {
                        return None;
                    }
                    let dependency = self.domains.dependency_fact(symbol)?;
                    if dependency.dependencies.is_empty()
                        || !self.domains.dependency_paths(dependency).all(|path| {
                            (0..=path.len()).all(|length| {
                                validation::project_type_reference_from_segments(
                                    self.program,
                                    source_type,
                                    &path[..length],
                                )
                                .is_some_and(|reference| {
                                    !crate::flow::operand_binds_live_view(self.program, reference)
                                })
                            })
                        })
                    {
                        return None;
                    }
                    let place = fact.place.as_place()?;
                    let place = crate::flow::canonical_place_from_semantic_place(
                        self.program,
                        self.semantic,
                        self.semantic.places.get(place),
                    )?;
                    crate::flow::canonical_places_agree(self.program, &source, &place)
                        .then_some((symbol, identity))
                })
                .collect()
        }

        pub(super) fn append_result_facts(
            &mut self,
            expression: ExpressionHandle,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
                || matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Name(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Indexed(_)
                        | ExpressionNode::Borrow(_)
                )
            {
                return;
            }
            let Some(reference) = self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                expression,
            ) else {
                return;
            };
            let mut domains =
                validation::field_domains::domain_constraint_identities(self.program, reference);
            for observed in self.cast_predicate_observations(expression, reference, *contexts) {
                crate::shared::collections::push_unique(&mut domains, observed);
            }
            let mut predicates = Vec::new();

            if let Some(call) = self.program.expression_table.call_node(expression) {
                validation::normal_return_float_predicates(
                    self.program,
                    self.machine.symbol,
                    expression,
                    call,
                    &mut predicates,
                );
            }
            let integer_bounds = self
                .program
                .expression_table
                .call_node(expression)
                .and_then(|call| {
                    validation::normal_return_integer_bounds(
                        self.program,
                        self.machine.symbol,
                        expression,
                        call,
                    )
                })
                .and_then(|(minimum, maximum)| {
                    Some(facts::IntegerRange {
                        minimum: numerics::bignum::BigInt::from_i64(minimum?),
                        maximum: numerics::bignum::BigInt::from_i64(maximum?),
                    })
                });
            let zero_fields = if matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::StructLiteral(_)
            ) {
                validation::field_domains::declared_owned_field_domain_identities(self.program, reference)
                    .into_iter()
                    .filter(|(path, _, _)| {
                        crate::flow::literal_value_projections(self.program, expression, reference, path, false)
                            .is_some_and(|projections| !projections.is_empty() && projections.iter().all(|projection| {
                                matches!(projection.source, crate::flow::LiteralValueSource::OmittedZero { type_reference, .. }
                                    if !validation::type_requires_establishment(self.program, type_reference))
                            }))
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let field_bounds = validation::declared_float_field_bounds(self.program, reference);
            if domains.is_empty()
                && predicates.is_empty()
                && integer_bounds.is_none()
                && zero_fields.is_empty()
                && field_bounds.is_empty()
            {
                return;
            }
            let place = self.semantic.append_expression_place(expression);
            let point = ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            };
            let mut refs = HandleSpan::empty();
            if let Some(bounds) = integer_bounds {
                let bounds = self.semantic.integer_ranges.append(bounds);
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::propagated_evidence(self.state.symbol),
                    payload: FactPayload::AssignedIntegerBounds { bounds },
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            for (path, domain_symbol, semantic_domain) in zero_fields {
                let field = crate::published::semantic::places::append_place(
                    self.semantic,
                    facts::PlaceRoot::Expression(expression),
                    &path,
                );
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(field),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::checked_validation_evidence(
                        self.state.symbol,
                    ),
                    payload: FactPayload::DomainMembership {
                        value: expression,
                        domain: HandleSpan::empty(),
                        domain_symbol,
                        semantic_domain,
                    },
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            for (domain_symbol, semantic_domain) in domains {
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::propagated_evidence(self.state.symbol),
                    payload: FactPayload::DomainMembership {
                        value: expression,
                        domain: HandleSpan::empty(),
                        domain_symbol,
                        semantic_domain,
                    },
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            for bound in field_bounds {
                let Some(predicate) = bound.predicate() else {
                    continue;
                };
                let field = self.semantic.append_expression_place(expression);
                for symbol in bound.path {
                    self.semantic
                        .push_place_segment(field, facts::PlaceSegment::Field { symbol });
                }
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(field),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::checked_validation_evidence(
                        self.state.symbol,
                    ),
                    payload: FactPayload::FloatPredicate { predicate },
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            for predicate in predicates {
                let fact = self.semantic.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point,
                    origin: FactOrigin::StatementTransfer,
                    evidence: crate::published::facts::propagated_evidence(self.state.symbol),
                    payload: FactPayload::FloatPredicate { predicate },
                });
                self.semantic.append_ref(&mut refs, fact);
            }
            let context = self.semantic.append_context(point, refs);
            reference_spans::append_flow_reference(
                &mut self.context.contexts.semantic_context_refs,
                contexts,
                FlowSemanticContextRef { context },
            );
            append_constraint_ref(
                &mut self.context.contexts.constraint_refs,
                constraints,
                FlowConstraintKind::SemanticContext { context },
            );
        }
    }
}
mod result_memberships {
    /*
        Normal invocation promises belong to the completed value, with its live
        backing dependencies. A footprint describes invalidation, not a field path
        on the selected result. Neither a receiving type nor a loan issues a domain.
    */
    use super::Execution;
    use crate::published::semantic::places::append_place;
    use arena::{Handle, HandleSpan};
    use checked_trees::{FlowCallFact, FlowConstraintRef, FlowOperandFact, FlowSemanticContextRef};
    use facts::{Fact, FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint};

    impl Execution<'_, '_, '_> {
        pub(super) fn append_invocation_memberships(
            &mut self,
            invocation: Handle<FlowCallFact>,
            value: &mut FlowOperandFact,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            if !self.context.control.calls.is_valid(invocation) {
                return;
            }
            let call = self.context.control.calls.get(invocation);
            let target = call.target_symbol;
            let ordinal = call.call_ordinal;
            if call.authored_expression != value.expression
                || call.statement_index != self.statement_index
            {
                return;
            }
            let Some(source) = self.program.expression_table.call_node(value.expression) else {
                return;
            };
            let Some(produced) = self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                value.expression,
            ) else {
                return;
            };
            if validation::reference_shell(self.program, produced).is_none() {
                return;
            }
            let selected = self
                .program
                .requirement_call_signature(self.machine.symbol, source);
            let rows = crate::flow::calls::call_result_qualification_identities_indexed(
                self.program,
                self.context,
                target,
                selected,
                produced,
            );
            let realization = crate::flow::calls::checked_realization_result_identities(
                self.program,
                self.context,
                self.machine.symbol,
                source,
            );
            let point = ProgramPoint::CallEnsures {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
                call_ordinal: ordinal,
            };
            for row in rows {
                let (path, domain, identity) = row.identity.clone();
                if !crate::flow::calls::result_identity_is_available(
                    self.program,
                    self.machine.symbol,
                    value.expression,
                    source,
                    realization.as_deref().map(Vec::as_slice),
                    &row,
                ) {
                    continue;
                }
                let dependencies = self
                    .membership_backing_dependencies(value, &path)
                    .unwrap_or_else(|| vec![FactPlace::Unknown]);
                let place = append_place(
                    self.semantic,
                    PlaceRoot::Expression(value.expression),
                    &path,
                );
                let context = crate::published::semantic::facts::append_observation_context(
                    self.semantic,
                    Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin: FactOrigin::CallEnsures,
                        evidence: crate::published::facts::propagated_evidence(target),
                        payload: FactPayload::DomainMembership {
                            value: value.expression,
                            domain: HandleSpan::empty(),
                            domain_symbol: domain,
                            semantic_domain: identity,
                        },
                    },
                    &dependencies,
                );
                self.context
                    .append_semantic_context(contexts, constraints, context);
            }
            value.constraints = *constraints;
        }
        fn membership_backing_dependencies(
            &mut self,
            value: &FlowOperandFact,
            path: &[facts::PlaceSegment],
        ) -> Option<Vec<FactPlace>> {
            let shell = validation::reference_shell(self.program, value.type_reference)?;
            let selected =
                validation::owned_projection_type_reference(self.program, shell.referee, path)?;
            if validation::reference_shell(self.program, selected).is_some()
                || validation::borrow_contracts::is_borrow_carrying_data(self.program, selected)
            {
                return None;
            }
            let mut dependencies = Vec::new();
            for backing in self.completed_backing_places(value)? {
                if !backing.root.is_valid()
                    || backing.root == PlaceRoot::Expression(value.expression)
                {
                    return None;
                }
                let place = crate::flow::project_captured_view_place(backing, path)?;
                let dependency =
                    FactPlace::Place(append_place(self.semantic, place.root, &place.segments));
                crate::shared::collections::push_unique(&mut dependencies, dependency);
            }
            (!dependencies.is_empty()).then_some(dependencies)
        }
    }
}
mod returned_descriptors {
    /*
        Keep a returned view tied to the inputs actually evaluated for its call.
        Exact projections retain descriptor candidates. Computed ranges retain the
        lending descriptor and enclosing backing without inventing an offset.
        The resource owner still checks every captured borrowing parent separately.
        Declared lifetimes retain every owed input; pre-call contents are discarded.
        A saved descriptor keeps its invocation-entry identity after its slot changes.
        Public local-dispatch regions supply exclusion coverage alone.
    */
    use super::Execution;
    use arena::Handle;
    use checked_trees::{FlowOperandCompletion, FlowOperandFact, FlowOperandRetention};
    use facts::PlaceSegment;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    impl Execution<'_, '_, '_> {
        pub(super) fn capture_returned_descriptor(
            &mut self,
            invocation: Handle<checked_trees::FlowCallFact>,
            call: &typed_trees::expression::TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            returned: TypeReferenceHandle,
            path: &[PlaceSegment],
            retention: Handle<FlowOperandRetention>,
            source_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
        ) -> Handle<FlowOperandFact> {
            if !self.context.control.calls.is_valid(invocation) {
                return Handle::invalid();
            }
            let completion = FlowOperandCompletion::InvocationResult {
                invocation,
                result_type: returned,
                path: self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(path.iter().copied()),
            };
            let Some(reference) =
                validation::project_type_reference_from_segments(self.program, returned, path)
                    .filter(|reference| {
                        validation::reference_shell(self.program, *reference).is_some()
                    })
            else {
                return Handle::invalid();
            };
            let actual = self.context.control.calls.get(invocation);
            let completed_result = FlowOperandFact {
                completion,
                type_reference: reference,
                expression: actual.authored_expression,
                state_symbol: self.state.symbol,
                statement_index: actual.statement_index,
                storage_snapshot: self.storage_snapshot,
                constraints: actual.exit_constraints,
                ..Default::default()
            };
            self.returned_descriptor(
                call,
                inputs,
                returned,
                path,
                retention,
                source_snapshot,
                completed_result,
            )
            .or_else(|| {
                self.capture_returned_region_descriptor(
                    call,
                    inputs,
                    returned,
                    path,
                    retention,
                    source_snapshot,
                    completed_result,
                )
            })
            .unwrap_or_else(|| {
                // Keep the performed call even when its geometry is unavailable here.
                // Checked return sources still have to establish its borrowing permission.
                self.context.control.operands.insert(FlowOperandFact {
                    retention,
                    ..completed_result
                })
            })
        }

        fn returned_descriptor(
            &mut self,
            call: &typed_trees::expression::TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            returned: TypeReferenceHandle,
            path: &[PlaceSegment],
            retention: Handle<FlowOperandRetention>,
            source_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
            completed_result: FlowOperandFact,
        ) -> Option<Handle<FlowOperandFact>> {
            let target = validation::call_target_symbol(self.program, call);
            let state = validation::call_target_state(self.program, target)?;
            let parameters = self.program.state_parameters(state);
            let sources = self
                .context
                .call_frames?
                .reference_result_leaf_origins_for_arguments(
                    state.symbol,
                    self.program
                        .expression_table
                        .expression_handles(call.arguments),
                )?;
            let sources = erase_callee_selectors(self.program, sources);
            let reference =
                validation::project_type_reference_from_segments(self.program, returned, path)?;
            let expected = validation::reference_shell(self.program, reference)?;
            let mut roots = self.context.control.retained_roots(retention)?.to_vec();
            let mut places = Vec::new();
            let mut candidates = Vec::new();
            let mut has_unknown_geometry = false;
            let mut all_image = true;
            for source in sources {
                if self.returned_slot_path(returned, &source.result_path)? != path {
                    continue;
                }
                if source.source.is_image_storage {
                    has_unknown_geometry = true;
                    candidates.push((Vec::new(), true, None, false));
                    continue;
                }
                if !source.source.recasts.is_empty()
                    || source.source.representation.is_some()
                    || source.source.segments.iter().any(|segment| {
                        !super::super::place_segment_has_static_identity(segment)
                            && !matches!(
                                segment,
                                PlaceSegment::OpaqueRange | PlaceSegment::Index { .. }
                            )
                    })
                {
                    return None;
                }
                let has_exact_projection = source
                    .source
                    .segments
                    .iter()
                    .all(super::super::place_segment_has_static_identity);
                let position = typed_trees::signature::state_parameter_position(
                    parameters,
                    source.source.root,
                )?;
                inputs.get(position)?;
                let FlowOperandCompletion::InvocationResult { invocation, .. } =
                    completed_result.completion
                else {
                    return None;
                };
                let actual = self
                    .context
                    .control
                    .calls
                    .get(invocation)
                    .argument_operands()
                    .handles()
                    .nth(position)?;
                let values =
                    self.call_backing_values(actual, &source.source, reference, source_snapshot)?;
                for (operand, projection) in values {
                    let value = *self.context.control.operands.get(operand);
                    let supplied = validation::project_type_reference_from_segments(
                        self.program,
                        value.type_reference,
                        &projection,
                    )?;
                    let supplied = validation::reference_shell(self.program, supplied)?;
                    if !(supplied.access == expected.access
                        || supplied.access == language_semantics::ReferenceAccess::Mutable)
                        || !validation::storage_origin_types_match(
                            self.program,
                            supplied.referee,
                            expected.referee,
                        )
                    {
                        return None;
                    }
                    let profiles =
                        validation::completed_storage::captured_retention_profiles_at_path(
                            self.program,
                            &self.context.control,
                            &value,
                            &projection,
                        )?;
                    let image = !profiles.is_empty()
                        && profiles.iter().all(|profile| {
                            self.context.control.operand_retentions.is_valid(*profile)
                                && self
                                    .context
                                    .control
                                    .operand_retentions
                                    .get(*profile)
                                    .immutable_image
                        });
                    for profile in profiles {
                        for root in self.context.control.retained_roots(profile)? {
                            if !roots.contains(root) {
                                roots.push(*root);
                            }
                        }
                    }
                    let selected = has_exact_projection
                        .then(|| {
                            super::super::operand_values::captured_operand_reference_places_with_formation(
                                self.program,
                                &self.context.control,
                                &value,
                                value.type_reference,
                                &projection,
                            )
                        })
                        .flatten();
                    if selected.as_ref().is_some_and(Vec::is_empty) && !image {
                        return None;
                    }
                    has_unknown_geometry |= image || selected.is_none();
                    let selected = selected.unwrap_or_default();
                    for place in &selected {
                        if !places.contains(place) {
                            places.push(place.clone());
                        }
                    }
                    all_image &= image;
                    candidates.push((
                        selected,
                        image,
                        Some((operand, projection)),
                        has_exact_projection,
                    ));
                }
            }
            if candidates.is_empty() {
                return None;
            }
            let mut alternatives = Vec::new();
            for (places, image, supplier, has_exact_projection) in candidates {
                let retention = self.append_retention(&roots);
                self.context
                    .control
                    .operand_retentions
                    .get_mut(retention)
                    .immutable_image = image;
                if let Some((operand, projection)) = supplier {
                    let value = *self.context.control.operands.get(operand);
                    let copied_sources = if self
                        .context
                        .control
                        .operand_retentions
                        .is_valid(value.retention)
                    {
                        self.context
                            .control
                            .operand_retentions
                            .get(value.retention)
                            .reborrow_sources
                    } else {
                        Default::default()
                    };
                    let reborrow_sources = if has_exact_projection
                        && !image
                        && copied_sources.is_empty()
                        && !projection.is_empty()
                        && validation::reference_shell(self.program, value.type_reference).is_some()
                    {
                        let suffix = self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(projection.iter().copied());
                        self.context.control.operand_reborrow_sources.insert_many([
                            checked_trees::FlowOperandReborrowSource {
                                operand,
                                source_path: Default::default(),
                                projection: suffix,
                            },
                        ])
                    } else {
                        copied_sources
                    };
                    let projection = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(projection);
                    let suppliers = if has_exact_projection {
                        self.context
                            .control
                            .operand_retained_suppliers
                            .insert_many([checked_trees::FlowOperandRetainedSupplier {
                                contract: None,
                                operand,
                                projection,
                            }])
                    } else {
                        Default::default()
                    };
                    let enclosing_regions = if has_exact_projection {
                        Default::default()
                    } else {
                        if reborrow_sources.is_empty() {
                            return None;
                        }
                        let regions = self
                            .context
                            .control
                            .operand_reborrow_sources
                            .span_or_empty(reborrow_sources)
                            .iter()
                            .map(|source| checked_trees::FlowOperandRetainedRegion {
                                operand: source.operand,
                                projection: source.source_path,
                            })
                            .collect::<Vec<_>>();
                        self.context
                            .control
                            .operand_retained_regions
                            .insert_many(regions)
                    };
                    let profile = self.context.control.operand_retentions.get_mut(retention);
                    profile.suppliers = suppliers;
                    profile.descriptor_sources = suppliers;
                    profile.reborrow_sources = reborrow_sources;
                    profile.enclosing_regions = enclosing_regions;
                }
                let referents = self.append_captured_references(&places);
                alternatives.push(FlowOperandFact {
                    type_reference: reference,
                    referents,
                    retention,
                    ..completed_result
                });
            }
            if alternatives.len() == 1 {
                return Some(self.context.control.operands.insert(alternatives.pop()?));
            }
            let mut profiles = Vec::new();
            for value in alternatives {
                let descriptor = self.context.control.operands.insert(value);
                let mut profile = *self.context.control.operand_retentions.get(value.retention);
                profile.descriptor = descriptor;
                profiles.push(profile);
            }
            let alternatives = self
                .context
                .control
                .operand_retentions
                .insert_many(profiles);
            let referents = if has_unknown_geometry {
                Default::default()
            } else {
                self.append_captured_references(&places)
            };
            let retention = self.append_retention(&roots);
            self.context
                .control
                .operand_retentions
                .get_mut(retention)
                .immutable_image = all_image;
            self.context
                .control
                .operand_retentions
                .get_mut(retention)
                .alternatives = alternatives;
            Some(self.context.control.operands.insert(FlowOperandFact {
                type_reference: reference,
                referents,
                retention,
                ..completed_result
            }))
        }

        pub(super) fn returned_slot_path(
            &self,
            returned: TypeReferenceHandle,
            path: &[PlaceSegment],
        ) -> Option<Vec<PlaceSegment>> {
            let owner = crate::borrow::owner_path_from_place_segments(self.program, path);
            let structural = crate::borrow::structural_owner_path(self.program, &owner)?;
            validation::reference_result_custody::loan_place_segments(
                self.program,
                returned,
                &structural,
            )
        }

        /*
            Complete body-derived image leaves, independent of the declaration's
            caller-lifetime inputs. A static alternative alone is insufficient.
        */
        pub(super) fn returned_image_paths(
            &self,
            call: &typed_trees::expression::TableCallExpression,
            returned: TypeReferenceHandle,
        ) -> Option<Vec<Vec<PlaceSegment>>> {
            let target = validation::call_target_symbol(self.program, call);
            let state = validation::call_target_state(self.program, target)?;
            let sources = self
                .context
                .call_frames?
                .reference_result_leaf_origins(state.symbol)?;
            let mut paths = Vec::new();
            for source in &sources {
                if !source.source.is_image_storage
                    || sources.iter().any(|other| {
                        other.result_path == source.result_path && !other.source.is_image_storage
                    })
                {
                    continue;
                }
                let path = self.returned_slot_path(returned, &source.result_path)?;
                crate::shared::collections::push_unique(&mut paths, path);
            }
            Some(paths)
        }
    }

    fn erase_callee_selectors(
        program: &dyn validation::CheckingInput,
        sources: Vec<validation::ReferenceResultLeafOrigin>,
    ) -> Vec<validation::ReferenceResultLeafOrigin> {
        sources
            .into_iter()
            .map(|mut source| {
                source.source.segments = source
                    .source
                    .segments
                    .into_iter()
                    .map(|segment| {
                        segment.erase_runtime_selector(|expression| {
                            program.expression_table.range_node(expression).is_some()
                        })
                    })
                    .collect();
                source
            })
            .collect()
    }
}
mod returned_regions {
    /*
        Completed results retain body-derived or public local-dispatch regions.
        Every table realization must refine its public WithinRegion contract;
        lifetime tags alone, particularly provider-owned views, promise no such
        geometry. Keep these regions separate from exact descriptor suppliers:
        exclusion and write invalidation may use them, but scalar identity, contents
        and permission ancestry may not. One unresolved region withholds coverage.
    */
    use super::Execution;
    use arena::Handle;
    use checked_trees::{
        CheckedReferenceOriginProjection as OriginProjection, CheckedReferenceOriginRoot,
        CheckedUnitStructuralPathSegment, FlowOperandFact, FlowOperandRetainedRegion,
        FlowOperandRetention,
    };
    use facts::PlaceSegment;
    use semantic_vocabulary::ReferenceOriginExtent;
    use typed_trees::expression::{ExpressionHandle, TableCallExpression};
    use typed_trees::types::TypeReferenceHandle;

    impl Execution<'_, '_, '_> {
        pub(super) fn capture_returned_region_descriptor(
            &mut self,
            call: &TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            returned: TypeReferenceHandle,
            path: &[PlaceSegment],
            retention: Handle<FlowOperandRetention>,
            source_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
            completed_result: FlowOperandFact,
        ) -> Option<Handle<FlowOperandFact>> {
            let (parameters, origins) =
                region_sources(self.program, call, self.context.call_frames, returned, path)?;
            let reference =
                validation::project_type_reference_from_segments(self.program, returned, path)?;
            validation::reference_shell(self.program, reference)?;
            let mut regions = Vec::new();
            for origin in origins {
                let output_path = validation::reference_result_custody::loan_place_segments(
                    self.program,
                    returned,
                    &origin.path,
                )?;
                if output_path != path {
                    continue;
                }
                if origin.source.extent != ReferenceOriginExtent::WithinRegion {
                    return None;
                }
                let CheckedReferenceOriginRoot::Parameter { index } = origin.source.root else {
                    return None;
                };
                let (position, parameter) = validation::reference_result_custody::origin_formal(
                    self.program,
                    parameters,
                    index,
                )?;
                let (_, actual) = inputs.get(position)?;
                let projection = validation::reference_result_custody::origin_place_segments(
                    self.program,
                    actual.type_reference,
                    origin
                        .source
                        .path
                        .iter()
                        .filter(|projection| {
                            !matches!(
                                projection,
                                OriginProjection::Exact(CheckedUnitStructuralPathSegment::Referent)
                            )
                        })
                        .cloned(),
                )?;
                if !validation::storage_origin_types_match(
                    self.program,
                    actual.type_reference,
                    parameter.type_reference,
                ) {
                    return None;
                }
                let expected = region_descriptor_type(
                    self.program,
                    parameter.type_reference,
                    &origin.source.path,
                )?;
                let checked_trees::FlowOperandCompletion::InvocationResult { invocation, .. } =
                    completed_result.completion
                else {
                    return None;
                };
                let actual = self
                    .context
                    .control
                    .calls
                    .get(invocation)
                    .argument_operands()
                    .handles()
                    .nth(position)?;
                for (operand, projection) in
                    self.call_slot_values(actual, &projection, source_snapshot)?
                {
                    let capture = self.context.control.operands.get(operand);
                    let selected = validation::project_type_reference_from_segments(
                        self.program,
                        capture.type_reference,
                        &projection,
                    )?;
                    validation::reference_shell(self.program, selected)?;
                    if !validation::storage_origin_types_match(self.program, selected, expected) {
                        return None;
                    }
                    let projection = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(projection);
                    regions.push(FlowOperandRetainedRegion {
                        operand,
                        projection,
                    });
                }
            }
            if regions.is_empty() || !self.context.control.operand_retentions.is_valid(retention) {
                return None;
            }
            let profile = *self.context.control.operand_retentions.get(retention);
            let enclosing_regions = self
                .context
                .control
                .operand_retained_regions
                .insert_many(regions);
            let retention = self
                .context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    enclosing_regions,
                    ..profile
                });
            Some(self.context.control.operands.insert(FlowOperandFact {
                type_reference: reference,
                retention,
                ..completed_result
            }))
        }
    }

    /*
        Choose geometric authority before binding it to completed actuals. A public
        local slot supplies its admitted interface; an ordinary call supplies its
        complete body relation. An opaque/provider signature supplies neither.
    */
    fn region_sources<'program>(
        program: &'program dyn validation::CheckingInput,
        call: &TableCallExpression,
        frames: Option<&validation::CallFrameResolver<'_>>,
        returned: TypeReferenceHandle,
        requested_path: &[PlaceSegment],
    ) -> Option<(
        &'program [typed_trees::signature::StateParameter],
        Vec<checked_trees::CheckedReturnedReferencePlan>,
    )> {
        let target = validation::call_target_symbol(program, call);
        if let Some(slot) = program.dynamic_slot_by_symbol(target) {
            return Some((
                &slot.parameters,
                validation::reference_result_custody::declared_return_sources(
                    program,
                    &slot.parameters,
                    slot.return_type,
                )
                .ok()?,
            ));
        }
        let state = validation::call_target_state(program, target)?;
        let machine = program.machine_holding_state(state.symbol)?;
        if !machine.body_is_present
            || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        {
            return None;
        }
        let parameters = program.state_parameters(state);
        let mut origins = Vec::new();
        for origin in frames?.reference_result_leaf_origins(state.symbol)? {
            let (output, selected, _) =
                validation::reference_result_custody::reference_origin_path(
                    program,
                    state.return_type,
                    &origin.result_path,
                )?;
            let output = output
                .into_iter()
                .map(|projection| match projection {
                    OriginProjection::Exact(segment) => Some(segment),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            if validation::reference_result_custody::loan_place_segments(
                program, returned, &output,
            )? != requested_path
            {
                continue;
            }

            if origin.source.is_image_storage
                || !origin.source.recasts.is_empty()
                || origin.source.representation.is_some()
            {
                return None;
            }
            let (index, parameter) = validation::reference_result_custody::origin_index(
                program,
                parameters,
                origin.source.root,
            )?;
            let (entry_reference_hops, projection, endpoint, extent) =
                validation::reference_result_custody::parameter_reference_origin_path(
                    program,
                    parameter.type_reference,
                    &origin.source.segments,
                )?;
            let extent = match extent {
                ReferenceOriginExtent::WithinRegion => extent,
                ReferenceOriginExtent::Projected
                    if validation::reference_shell(program, endpoint).is_some() =>
                {
                    ReferenceOriginExtent::WithinRegion
                }
                _ => return None,
            };
            origins.push(checked_trees::CheckedReturnedReferencePlan {
                path: output,
                source: checked_trees::CheckedReferenceOriginSourcePlan {
                    root: CheckedReferenceOriginRoot::Parameter { index },
                    entry_reference_hops,
                    path: projection,
                    extent,
                    type_identity: program.normalized_type_identity(endpoint),
                    access: validation::reference_shell(program, selected)?
                        .access
                        .into(),
                },
            });
        }
        Some((parameters, origins))
    }

    /*
        A regional source may end at a descriptor or its referent. Preserve every
        explicit reference crossing while finding that descriptor's declared type:
        `&&[u8]` enclosing a stored `&[u8]` is not the latter's backing region.
    */
    fn region_descriptor_type(
        program: &dyn validation::CheckingInput,
        mut reference: TypeReferenceHandle,
        path: &[OriginProjection],
    ) -> Option<TypeReferenceHandle> {
        let project = |reference, pending: &[OriginProjection]| {
            let segments = validation::reference_result_custody::origin_place_segments(
                program,
                reference,
                pending.iter().cloned(),
            )?;
            validation::project_type_reference_from_segments(program, reference, &segments)
        };
        let mut pending = Vec::new();
        let mut descriptor = TypeReferenceHandle::invalid();
        for projection in path {
            if matches!(
                projection,
                OriginProjection::Exact(CheckedUnitStructuralPathSegment::Referent)
            ) {
                reference = project(reference, &pending)?;
                pending.clear();
                descriptor = reference;
                reference = validation::reference_shell(program, reference)?.referee;
            } else {
                pending.push(projection.clone());
            }
        }
        let ended_at_referent = pending.is_empty() && descriptor.is_valid();
        reference = project(reference, &pending)?;
        if validation::reference_shell(program, reference).is_some() {
            Some(reference)
        } else {
            ended_at_referent.then_some(descriptor)
        }
    }
}
mod selected_call_stores {
    /*
        Resource footprints of an exact completed local selector. Dispatch and
        guarantees remain public. Unknown values, differing alternatives and absent
        checked bodies supply no relation; constructor inventory is never closure.
    */
    use super::Execution;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowCallFact, DynamicConformanceFacts, FlowControlFacts, FlowOperandFact,
        FlowStatementFact,
    };
    use facts::PlaceSegment;
    use symbols::SymbolHandle;
    use typed_trees::{machine::Machine, state::State, types::TypeReferenceHandle};

    impl<'a, 'b, 'plans> Execution<'a, 'b, 'plans> {
        pub(super) fn selected_store_target(
            &mut self,
            call: &BorrowCallFact,
            actuals: HandleSpan<FlowOperandFact>,
        ) -> Option<(&'plans Machine, &'plans State, HandleSpan<FlowOperandFact>)> {
            if let Some(state) = validation::call_target_state(self.program, call.target_symbol) {
                return Some((
                    self.program.machine_holding_state(state.symbol)?,
                    state,
                    actuals,
                ));
            }
            let slot = self.program.dynamic_slot_by_symbol(call.target_symbol)?;
            let prefix = slot
                .parameters
                .len()
                .checked_sub(slot.data_parameters().len())?;
            if prefix != 1 || actuals.len() != slot.parameters.len() {
                return None;
            }
            let selector = actuals.handle_at(0);
            let statements = super::super::appended_span_since(
                &self.context.control.statements,
                self.context.state_statements_start,
            );
            let implementation = completed_selection(
                self.program,
                &self.context.control,
                self.context.dynamic_conformances.as_ref()?,
                selector,
                slot.parameters.first()?.type_reference,
                &[],
                Some((self.state.symbol, statements)),
            )?;
            let conformance = self.program.conformance_by_symbol(implementation)?;
            if !conformance.type_parameters.is_empty() {
                return None;
            }
            let mut rows = self
                .context
                .dynamic_conformances
                .as_ref()?
                .selections
                .iter()
                .filter(|selection| {
                    selection.implementation == implementation
                        && selection.interface_identity == slot.interface_identity
                })
                .flat_map(|selection| &selection.rows)
                .filter(|row| {
                    row.declaring_trait == slot.declaring_trait
                        && row.requirement == slot.requirement
                });
            let row = rows.next()?;
            if !row.family_tuple.is_empty() || rows.any(|other| other != row) {
                return None;
            }
            let machine = self.program.machine_by_symbol(row.realization_machine)?;
            let state = self.program.state_by_symbol(row.realization_state)?;
            if !machine.body_is_present
                || !machine.supply_mode.is_checked_body()
                || machine.target_family_interface
                || self.program.machine_states(machine).first()?.symbol != state.symbol
                || !self.program.machine_type_parameters(machine).is_empty()
            {
                return None;
            }
            let parameters = self.program.state_parameters(state);
            let public = slot.data_parameters();
            if parameters.len() != public.len()
                || !parameters.iter().zip(public).all(|(private, public)| {
                    private.is_const == public.is_const
                        && private.is_mutable == public.is_mutable
                        && validation::declared_carriers_match(
                            self.program,
                            private.type_reference,
                            public.type_reference,
                        )
                })
                || !validation::declared_carriers_match(
                    self.program,
                    state.return_type,
                    slot.return_type,
                )
            {
                return None;
            }
            Some((
                machine,
                state,
                HandleSpan::from_parts_or_empty(
                    actuals.handle_at(prefix as u32),
                    actuals.count() - prefix as u32,
                ),
            ))
        }

        /*
            Parameter paths whose completed value is one dynamic implementation.
            Only a field the caller actually formed is recorded. An unknown field
            stays off the list so the body keeps the requirement ceiling there.
        */
        pub(super) fn known_dynamic_selectors(
            &mut self,
            parameters: &[typed_trees::signature::StateParameter],
            actuals: HandleSpan<FlowOperandFact>,
        ) -> Vec<(SymbolHandle, Vec<PlaceSegment>, SymbolHandle)> {
            if parameters.len() != actuals.count() as usize {
                return Vec::new();
            }
            let Some(receipts) = self.context.dynamic_conformances.as_ref() else {
                return Vec::new();
            };
            let statements = super::super::appended_span_since(
                &self.context.control.statements,
                self.context.state_statements_start,
            );
            let mut selectors = Vec::new();
            for (parameter, actual) in parameters.iter().zip(actuals.handles()) {
                let mut paths = Vec::new();
                dynamic_selector_paths(
                    self.program,
                    parameter.type_reference,
                    &mut Vec::new(),
                    &mut paths,
                );
                for (path, expected) in paths {
                    let Some(implementation) = completed_selection(
                        self.program,
                        &self.context.control,
                        receipts,
                        actual,
                        expected,
                        &path,
                        Some((self.state.symbol, statements)),
                    ) else {
                        continue;
                    };
                    selectors.push((parameter.symbol, path, implementation));
                }
            }
            selectors
        }
    }

    fn dynamic_selector_paths(
        program: &dyn validation::CheckingInput,
        type_reference: TypeReferenceHandle,
        prefix: &mut Vec<PlaceSegment>,
        paths: &mut Vec<(Vec<PlaceSegment>, TypeReferenceHandle)>,
    ) {
        use typed_trees::types::TypeReferenceNode;
        if !type_reference.is_valid() || prefix.len() > 8 {
            return;
        }
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::DynamicTrait { .. } if !prefix.is_empty() => {
                paths.push((prefix.clone(), reference));
            }
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => {
                let Some(definition) = program.data_definition_by_symbol(*symbol) else {
                    return;
                };
                for member in program.data_members(definition) {
                    let typed_trees::data::DataMember::Field(field) = member else {
                        continue;
                    };
                    prefix.push(PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    dynamic_selector_paths(program, field.type_reference, prefix, paths);
                    prefix.pop();
                }
            }
            _ => {}
        }
    }

    fn completed_selection(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        receipts: &DynamicConformanceFacts,
        value: Handle<FlowOperandFact>,
        expected: TypeReferenceHandle,
        projection: &[PlaceSegment],
        active_statements: Option<(SymbolHandle, HandleSpan<FlowStatementFact>)>,
    ) -> Option<SymbolHandle> {
        enum Work {
            Visit(Handle<FlowOperandFact>, Vec<PlaceSegment>),
            Leave,
        }
        let expected = program.normalized_type_identity(expected);
        let mut pending = vec![Work::Visit(value, projection.to_vec())];
        let mut active = Vec::new();
        let mut selected = None;
        while let Some(work) = pending.pop() {
            let Work::Visit(value, projection) = work else {
                active.pop()?;
                continue;
            };
            if !control.operands.is_valid(value) || active.contains(&(value, projection.clone())) {
                return None;
            }
            let capture = control.operands.get(value);
            let reference = validation::project_type_reference_from_segments(
                program,
                capture.type_reference,
                &projection,
            )?;
            if program.normalized_type_identity(reference) != expected {
                return None;
            }
            active.push((value, projection.clone()));
            pending.push(Work::Leave);
            if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion
            {
                let rows = control.operand_alternatives.span(alternatives)?;
                if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                    return None;
                }
                pending.extend(
                    rows.iter()
                        .map(|row| Work::Visit(row.value, projection.clone())),
                );
                continue;
            }
            if projection.is_empty() && !capture.value_source.is_valid() {
                let mut constructors = receipts.selections.iter().filter(|receipt| {
                    receipt.occurrence == capture.expression
                        && receipt.interface_identity == expected
                });
                if let Some(receipt) = constructors.next() {
                    if constructors.any(|other| other.implementation != receipt.implementation)
                        || selected.is_some_and(|selected| selected != receipt.implementation)
                    {
                        return None;
                    }
                    selected = Some(receipt.implementation);
                    continue;
                }
            }
            let predecessors = super::super::completed_values::predecessors_in_control(
                program,
                control,
                value,
                &projection,
                super::super::stored_values::StoredObservation::Binding,
                active_statements,
            )?;
            if predecessors.is_empty() {
                return None;
            }
            pending.extend(
                predecessors
                    .into_iter()
                    .map(|(value, projection)| Work::Visit(value, projection)),
            );
        }
        selected
    }
}
mod stored_call_values {
    /*
        Capture normal-return descriptor associations from the actual evaluated
        tuple before invocation writes, then publish them on the one immutable
        storage timeline. The checked relation supplies correspondence; retained
        captures supply geometry and formation, never source replay or new authority.
    */
    mod lifetime_sources {
        /*
            A helper's saved output can keep several inputs borrowed even when only one
            supplies its address. Save every input before the helper changes its output;
            reading that variable later would observe a different reference generation.
        */
        use super::super::Execution;
        use arena::{Handle, HandleSpan};
        use checked_trees::{
            BorrowCallFact, FlowOperandFact, FlowOperandRetainedSupplier, FlowOperandStoredSource,
            FlowStorageSnapshot,
        };
        use facts::PlaceSegment;

        impl Execution<'_, '_, '_> {
            pub(in crate::flow::expression) fn capture_stored_lifetime_sources(
                &mut self,
                input: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
                snapshot: Handle<FlowStorageSnapshot>,
            ) -> Option<HandleSpan<FlowOperandStoredSource>> {
                let actual = self.context.control.operands.get(input);
                let carrier = validation::reference_result_custody::input_carrier_type(
                    self.program,
                    actual.type_reference,
                )?;
                let path = validation::corresponding_place_projection(self.program, carrier, path)?;
                let selected =
                    validation::project_type_reference_from_segments(self.program, carrier, &path)?;
                let leaves = if validation::reference_shell(self.program, selected).is_some() {
                    vec![Vec::new()]
                } else {
                    validation::borrow_contracts::carried_reference_paths(self.program, selected)?
                        .into_iter()
                        .map(|(path, _)| crate::borrow::owner_place_segments(&path))
                        .collect()
                };
                if leaves.is_empty() {
                    return None;
                }
                let mut sources = Vec::new();
                for leaf in leaves {
                    let mut full_path = path.clone();
                    full_path.extend_from_slice(&leaf);
                    let selected = validation::completed_storage::stored_reference_sources_at_path(
                        self.program,
                        &self.context.control,
                        input,
                        &full_path,
                        snapshot,
                    );
                    if matches!(
                        selected,
                        validation::completed_storage::StoredOperand::Unavailable
                    ) && ::validation::trace_flags::check_sites()
                    {
                        let actual = self.context.control.operands.get(input);
                        eprintln!(
                            "CHECKSITE stored-lifetime-input-unavailable state={:?} statement={} input={input:?} expression={:?} type={:?} path={full_path:?} snapshot={snapshot:?} referents={:?} retention={:?}",
                            self.state.symbol,
                            self.statement_index,
                            actual.expression,
                            actual.type_reference,
                            self.context
                                .control
                                .operand_referents
                                .span_or_empty(actual.referents),
                            actual.retention
                        );
                        if self
                            .context
                            .control
                            .operand_retentions
                            .is_valid(actual.retention)
                        {
                            eprintln!(
                                "CHECKSITE stored-lifetime-input-profile input={input:?} profile={:?}",
                                self.context
                                    .control
                                    .operand_retentions
                                    .get(actual.retention)
                            );
                        }
                    }
                    let leaf = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(leaf);
                    self.append_stored_lifetime_sources(selected, leaf, &mut sources)?;
                }
                Some(
                    self.context
                        .control
                        .operand_stored_sources
                        .insert_many(sources),
                )
            }

            fn append_stored_lifetime_sources(
                &mut self,
                selected: validation::completed_storage::StoredOperand,
                path: HandleSpan<PlaceSegment>,
                sources: &mut Vec<FlowOperandStoredSource>,
            ) -> Option<()> {
                use validation::completed_storage::StoredOperand;
                let slot = |this: &mut Self, place: validation::CanonicalPlace| {
                    checked_trees::FlowOperandReferent {
                        root: place.root,
                        segments: this
                            .context
                            .control
                            .operand_referent_segments
                            .insert_many(place.segments),
                        ..Default::default()
                    }
                };
                match selected {
                    StoredOperand::Unavailable => return None,
                    StoredOperand::Alternatives(alternatives) => {
                        if alternatives.is_empty() {
                            return None;
                        }
                        for alternative in alternatives {
                            self.append_stored_lifetime_sources(alternative, path, sources)?;
                        }
                    }
                    StoredOperand::Incoming(place) => {
                        sources.push(FlowOperandStoredSource::Incoming {
                            path,
                            slot: slot(self, place),
                        })
                    }
                    StoredOperand::Value {
                        capture,
                        reference,
                        projection,
                        slot: place,
                    } => {
                        let projection = self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(projection);
                        let slot = place.map(|place| slot(self, place));
                        sources.push(FlowOperandStoredSource::Value {
                            path,
                            operand: capture,
                            reference,
                            projection,
                            slot,
                        });
                    }
                }
                Some(())
            }

            pub(super) fn capture_output_lifetime_sources(
                &mut self,
                parameters: &[typed_trees::signature::StateParameter],
                inputs: HandleSpan<FlowOperandFact>,
                call: &BorrowCallFact,
                identity: facts::InvocationOutputIdentity,
                owner_path: &[PlaceSegment],
            ) {
                let captured = (|| {
                    let output_parameter = parameters
                        .iter()
                        .position(|parameter| parameter.symbol == identity.parameter_symbol)?;
                    let relation = validation::borrow_contracts::resolve_provider_outputs(
                        self.program,
                        parameters,
                    )
                    .ok()?;
                    if relation.readiness
                        != validation::borrow_contracts::BorrowRelationReadiness::Complete
                    {
                        return None;
                    }
                    let output = relation.outputs.iter().find(|output| {
                        output.destination
                            == validation::borrow_contracts::ProviderOutputDestination::Parameter(
                                output_parameter,
                            )
                            && crate::borrow::owner_place_segments(&output.owner_path) == owner_path
                    })?;
                    let mut suppliers = Vec::new();
                    for source in &output.dependencies {
                        let input = inputs.handles().nth(source.parameter_index)?;
                        let source_path = crate::borrow::owner_place_segments(&source.source_path);
                        let stored_sources =
                            if source.source == checked_trees::ViewReturnInputSource::Stored {
                                self.capture_stored_lifetime_sources(
                                    input,
                                    &source_path,
                                    self.storage_snapshot,
                                )?
                            } else {
                                HandleSpan::empty()
                            };
                        let projection = self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(source_path.iter().copied());
                        let source_path = self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(source_path);
                        let owner_path = self
                            .context
                            .control
                            .operand_retained_paths
                            .insert_many(owner_path.iter().copied());
                        suppliers.push(FlowOperandRetainedSupplier {
                            operand: input,
                            projection,
                            contract: Some(checked_trees::FlowOperandLifetimeContract {
                                state_symbol: self.state.symbol,
                                statement_index: self.statement_index,
                                invocation: call.authored_expression,
                                call_ordinal: call.call_ordinal,
                                output_parameter: Some(output_parameter),
                                input,
                                parameter_index: source.parameter_index,
                                source: source.source,
                                source_type: source.source_type,
                                source_path,
                                owner_path,
                                stored_sources,
                            }),
                        });
                    }
                    Some(
                        self.context
                            .control
                            .operand_retained_suppliers
                            .insert_many(suppliers),
                    )
                })();
                let owner_path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(owner_path.iter().copied());
                self.context.control.output_lifetime_sources.append(
                    checked_trees::FlowOutputLifetimeSources {
                        output: identity,
                        owner_path,
                        sources: captured,
                    },
                );
            }
        }
    }
    mod suppliers {
        /*
            Checked supplier capture from immutable completed operands.
            Every destination is captured before publishing any write. Unknown or
            lifetime-only suppliers cannot manufacture an address or current-slot read.
        */
        use super::super::Execution;
        use super::super::reference_projection::ReferenceProjection;
        use super::{CallStore, CallStoredValue};
        use arena::{Handle, HandleSpan};
        use checked_trees::{BorrowCallFact, FlowOperandFact};
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;

        impl Execution<'_, '_, '_> {
            pub(in super::super) fn capture_stored_suppliers(
                &mut self,
                parameters: &[typed_trees::signature::StateParameter],
                effects: &[validation::StoredReferenceEffect],
                inputs: HandleSpan<FlowOperandFact>,
                call: &BorrowCallFact,
                binding_writes: Option<&[CanonicalPlace]>,
            ) -> Vec<CallStore> {
                if parameters.len() != inputs.len() {
                    return Vec::new();
                }
                let input = |symbol: SymbolHandle| {
                    typed_trees::signature::state_parameter_position(parameters, symbol)
                        .map(|position| inputs.handle_at(position as u32))
                };
                let mut values: Vec<CallStore> = Vec::new();
                for effect in effects.iter() {
                    let Some(access) = effect.reference_access() else {
                        continue;
                    };
                    if effect.sources.is_empty()
                        && !effect.incoming_identity
                        && !effect.has_static_values
                    {
                        continue;
                    }
                    if effect.incoming_identity && binding_writes.is_some_and(<[_]>::is_empty) {
                        continue;
                    }
                    let Some(actual) = input(effect.destination.local_symbol) else {
                        continue;
                    };
                    let actual = *self.context.control.operands.get(actual);
                    let carrier = validation::reference_shell(self.program, actual.type_reference)
                        .map_or(actual.type_reference, |reference| reference.referee);
                    let Some(reference) = validation::project_type_reference_from_segments(
                        self.program,
                        carrier,
                        &effect.destination.local_segments,
                    ) else {
                        continue;
                    };
                    if validation::reference_shell(self.program, reference)
                        .is_none_or(|shell| shell.access != access)
                    {
                        continue;
                    }
                    let Some(destinations) = self.capture_reference_projection(
                        &actual,
                        &effect.destination.local_segments,
                        ReferenceProjection::Destination,
                    ) else {
                        continue;
                    };
                    let mut origins = Vec::new();
                    let mut roots = Vec::new();
                    let mut known = !effect.has_static_values;
                    let mut lifetime_known = true;
                    let supplies_output = effect
                        .sources
                        .iter()
                        .any(|source| source.backing.invocation_output.is_some());
                    let mut suppliers = Vec::new();
                    let mut source_images = Vec::new();
                    let mut suppliers_known = effect.suppliers_complete && !supplies_output;
                    if suppliers_known {
                        for source in &effect.sources {
                            if source.lifetime_access.is_some() {
                                source_images.push(None);
                                continue;
                            }
                            let Some(actual) = input(source.backing.root) else {
                                suppliers_known = false;
                                break;
                            };
                            let Some(selected) = self.call_backing_values(
                                actual,
                                &source.backing,
                                reference,
                                self.storage_snapshot,
                            ) else {
                                suppliers_known = false;
                                break;
                            };
                            if selected.is_empty() {
                                suppliers_known = false;
                                break;
                            }
                            let mut image_profiles = Vec::new();
                            let mut is_image = source.carrier_symbol == source.backing.root
                                && source.carrier_segments == source.backing.segments;
                            for (operand, projection) in selected {
                                let value = *self.context.control.operands.get(operand);
                                let Some(supplied) =
                                    validation::project_type_reference_from_segments(
                                        self.program,
                                        value.type_reference,
                                        &projection,
                                    )
                                    .and_then(|reference| {
                                        validation::reference_shell(self.program, reference)
                                    })
                                else {
                                    suppliers_known = false;
                                    break;
                                };
                                let expected =
                                    validation::reference_shell(self.program, reference).unwrap();
                                if !(supplied.access == expected.access
                                    || supplied.access
                                        == language_semantics::ReferenceAccess::Mutable)
                                    || !validation::storage_origin_types_match(
                                        self.program,
                                        validation::peel_declared_constraints(
                                            self.program,
                                            supplied.referee,
                                        ),
                                        validation::peel_declared_constraints(
                                            self.program,
                                            expected.referee,
                                        ),
                                    )
                                {
                                    suppliers_known = false;
                                    break;
                                }
                                match crate::flow::operand_values::captured_retention_profiles_at_path(
                                    self.program,
                                    &self.context.control,
                                    &value,
                                    &projection,
                                ) {
                                    Some(profiles) if !profiles.is_empty() => {
                                        is_image &= profiles.iter().all(|profile| {
                                            self.context.control.operand_retentions.is_valid(*profile)
                                                && self
                                                    .context
                                                    .control
                                                    .operand_retentions
                                                    .get(*profile)
                                                    .immutable_image
                                        });
                                        image_profiles.extend(profiles);
                                    }
                                    _ => is_image = false,
                                }
                                suppliers.push((operand, projection));
                            }
                            if !suppliers_known {
                                break;
                            }
                            source_images.push(is_image.then_some(image_profiles));
                        }
                    }
                    if !effect.suppliers_complete && !supplies_output {
                        known = false;
                    }
                    for (source_index, source) in effect.sources.iter().enumerate() {
                        if let Some(Some(profiles)) = source_images.get(source_index) {
                            for profile in profiles {
                                if let Some(retained) =
                                    self.context.control.retained_roots(*profile)
                                {
                                    for root in retained {
                                        if !roots.contains(root) {
                                            roots.push(*root);
                                        }
                                    }
                                } else {
                                    lifetime_known = false;
                                }
                            }
                            continue;
                        }
                        if supplies_output && source.lifetime_access.is_some() {
                            continue;
                        }
                        if source.backing.invocation_output.is_some() {
                            let Some(parameter) = parameters.iter().find(|parameter| {
                                parameter.symbol == effect.destination.local_symbol
                            }) else {
                                known = false;
                                lifetime_known = false;
                                break;
                            };
                            let identity = facts::InvocationOutputIdentity {
                                state_symbol: self.state.symbol,
                                statement_index: self.statement_index,
                                call_ordinal: call.call_ordinal,
                                parameter_symbol: parameter.symbol,
                            };
                            let origin = crate::flow::CapturedReferencePlace {
                                place: CanonicalPlace {
                                    root: facts::PlaceRoot::InvocationOutput(identity),
                                    segments: effect.destination.local_segments.clone(),
                                },
                                formation: Handle::invalid(),
                            };
                            if source.lifetime_access.is_some()
                                || !identity.is_valid()
                                || source.backing.is_image_storage
                                || !source.backing.recasts.is_empty()
                                || source.backing.representation.is_some()
                                || !self.is_incoming_descriptor_place(&origin.place)
                            {
                                known = false;
                                lifetime_known = false;
                                break;
                            }
                            let retained =
                                crate::borrow::lifetime_contract::completed_output_retention(
                                    self.borrow,
                                    self.machine.symbol,
                                    self.state.symbol,
                                    self.statement_index,
                                    call,
                                    identity,
                                    parameter.type_reference,
                                    &crate::borrow::owner_path_from_place_segments(
                                        self.program,
                                        &effect.destination.local_segments,
                                    ),
                                );
                            let retained = match retained {
                                Ok(retained) => retained,
                                Err(reason) => {
                                    if ::validation::trace_flags::struct_trace() {
                                        eprintln!(
                                            "STRUCT supplied-output-retention-refused identity={identity:?} destination={:?} reason={reason}",
                                            effect.destination.local_segments
                                        );
                                    }
                                    known = false;
                                    lifetime_known = false;
                                    break;
                                }
                            };
                            for root in retained {
                                crate::shared::collections::push_unique(&mut roots, root);
                            }
                            self.capture_output_lifetime_sources(
                                parameters,
                                inputs,
                                call,
                                identity,
                                &effect.destination.local_segments,
                            );
                            crate::shared::collections::push_unique(&mut origins, origin);
                            continue;
                        }
                        let Some(backing) = input(source.backing.root) else {
                            known = false;
                            lifetime_known = false;
                            break;
                        };
                        let backing = *self.context.control.operands.get(backing);
                        if source.backing.is_image_storage
                            || !source.backing.recasts.is_empty()
                            || source.backing.representation.is_some()
                        {
                            known = false;
                            lifetime_known = false;
                            break;
                        }
                        let Some(observed) = self.capture_reference_projection(
                            &backing,
                            &source.backing.segments,
                            ReferenceProjection::Value,
                        ) else {
                            known = false;
                            lifetime_known = false;
                            break;
                        };
                        if source.carrier_symbol != source.backing.root
                            || source.carrier_segments != source.backing.segments
                        {
                            let Some(carrier) = input(source.carrier_symbol) else {
                                known = false;
                                lifetime_known = false;
                                break;
                            };
                            let carrier = *self.context.control.operands.get(carrier);
                            let Some(carrier) = self.capture_reference_projection(
                                &carrier,
                                &source.carrier_segments,
                                ReferenceProjection::Value,
                            ) else {
                                known = false;
                                lifetime_known = false;
                                break;
                            };
                            if observed.places.len() != 1 || observed.places != carrier.places {
                                if ::validation::trace_flags::check_sites() {
                                    eprintln!(
                                        "CHECKSITE stored-reference-source-correspondence machine={:?} state={:?} statement={} call={:?} observed_count={} carrier_count={} observed={:?} carrier={:?}",
                                        self.machine.symbol,
                                        self.state.symbol,
                                        self.statement_index,
                                        call.authored_expression,
                                        observed.places.len(),
                                        carrier.places.len(),
                                        observed
                                            .places
                                            .iter()
                                            .take(4)
                                            .map(|place| (
                                                place.place.root,
                                                place.formation,
                                                place.place.segments.len(),
                                                &place.place.segments
                                                    [..place.place.segments.len().min(32)],
                                            ))
                                            .collect::<Vec<_>>(),
                                        carrier
                                            .places
                                            .iter()
                                            .take(4)
                                            .map(|place| (
                                                place.place.root,
                                                place.formation,
                                                place.place.segments.len(),
                                                &place.place.segments
                                                    [..place.place.segments.len().min(32)],
                                            ))
                                            .collect::<Vec<_>>(),
                                    );
                                }
                                known = false;
                                lifetime_known = false;
                                break;
                            }
                            if let Some(retained) =
                                self.context.control.retained_roots(carrier.retention)
                            {
                                for root in retained {
                                    if !roots.contains(root) {
                                        roots.push(*root);
                                    }
                                }
                            } else {
                                lifetime_known = false;
                            }
                        }
                        if let Some(retained) =
                            self.context.control.retained_roots(observed.retention)
                        {
                            for root in retained {
                                if !roots.contains(root) {
                                    roots.push(*root);
                                }
                            }
                        } else {
                            lifetime_known = false;
                        }
                        if source.lifetime_access.is_none() {
                            for origin in observed.places {
                                crate::shared::collections::push_unique(&mut origins, origin);
                            }
                        }
                    }
                    let has_image =
                        effect.has_static_values || source_images.iter().any(Option::is_some);
                    let has_local = effect.sources.iter().enumerate().any(|(position, source)| {
                        source.lifetime_access.is_none()
                            && source_images.get(position).is_none_or(Option::is_none)
                    });
                    if has_image && has_local {
                        known = false;
                    }
                    if (!known || origins.is_empty()) && ::validation::trace_flags::check_sites() {
                        eprintln!(
                            "CHECKSITE stored-reference-capture-unavailable machine={:?} state={:?} statement={} call={:?} target={:?} destination={:?}:path_count={}:prefix={:?} known={} origins={} sources={} incoming_identity={} retains_incoming={}",
                            self.machine.symbol,
                            self.state.symbol,
                            self.statement_index,
                            call.authored_expression,
                            call.target_symbol,
                            effect.destination.local_symbol,
                            effect.destination.local_segments.len(),
                            &effect.destination.local_segments
                                [..effect.destination.local_segments.len().min(32)],
                            known,
                            origins.len(),
                            effect.sources.len(),
                            effect.incoming_identity,
                            effect.retains_incoming,
                        );
                        for source in effect.sources.iter().take(8) {
                            eprintln!(
                                "CHECKSITE stored-reference-capture-source carrier={:?}:path_count={}:prefix={:?} backing={:?}:path_count={}:prefix={:?} lifetime_only={:?} image={} coarse={} recasts={} representation={}",
                                source.carrier_symbol,
                                source.carrier_segments.len(),
                                &source.carrier_segments[..source.carrier_segments.len().min(32)],
                                source.backing.root,
                                source.backing.segments.len(),
                                &source.backing.segments[..source.backing.segments.len().min(32)],
                                source.lifetime_access,
                                source.backing.is_image_storage,
                                source.backing.collection_coarse,
                                source.backing.recasts.len(),
                                source.backing.representation.is_some(),
                            );
                        }
                    }
                    let suppliers = if suppliers_known {
                        let rows = suppliers
                            .into_iter()
                            .map(
                                |(value, projection)| checked_trees::FlowOperandRetainedSupplier {
                                    contract: None,
                                    operand: value,
                                    projection: self
                                        .context
                                        .control
                                        .operand_retained_paths
                                        .insert_many(projection),
                                },
                            )
                            .collect::<Vec<_>>();
                        self.context
                            .control
                            .operand_retained_suppliers
                            .insert_many(rows)
                    } else {
                        HandleSpan::empty()
                    };
                    let weak_destination = destinations.places.len() != 1;
                    for destination in destinations.places {
                        if effect.incoming_identity
                            && !crate::flow::stored_values::invalidates_destination(
                                self.program,
                                binding_writes,
                                &destination.place,
                            )
                        {
                            continue;
                        }
                        let mut alternatives = Vec::new();
                        if !effect.sources.is_empty() || !effect.incoming_identity {
                            let referents = if known && !origins.is_empty() {
                                self.append_captured_references(&origins)
                            } else {
                                HandleSpan::empty()
                            };
                            let retention = if lifetime_known {
                                self.append_retention(&roots)
                            } else {
                                Handle::invalid()
                            };
                            if self.context.control.operand_retentions.is_valid(retention) {
                                let profile =
                                    self.context.control.operand_retentions.get_mut(retention);
                                profile.immutable_image = suppliers_known
                                    && effect
                                        .sources
                                        .iter()
                                        .all(|source| source.lifetime_access.is_none())
                                    && source_images.len() == effect.sources.len()
                                    && source_images.iter().all(Option::is_some)
                                    && (effect.has_static_values || !effect.sources.is_empty());
                                profile.suppliers = suppliers;
                            }
                            let descriptor_loan = if known {
                                self.supplied_descriptor_permission(call, &origins, reference)
                            } else {
                                Handle::invalid()
                            };
                            if supplies_output
                                && !descriptor_loan.is_valid()
                                && ::validation::trace_flags::struct_trace()
                            {
                                eprintln!(
                                    "STRUCT supplied-descriptor-binding-refused known={known} type={reference:?} origins={origins:?} completed={:?}",
                                    call.completed_reference_loans
                                        .handles()
                                        .filter_map(|handle| {
                                            let loan = self.borrow.loans.get(handle);
                                            (loan.lineage
                                                == checked_trees::BorrowLoanLineage::SuppliedOutput)
                                                .then(|| {
                                                    let incoming = self
                                                        .borrow
                                                        .incoming_references
                                                        .get(loan.incoming_reference);
                                                    (
                                                        handle,
                                                        &incoming.origin,
                                                        incoming.carrier_type,
                                                        self.borrow
                                                            .owner_segments
                                                            .span_or_empty(incoming.owner_path),
                                                    )
                                                })
                                        })
                                        .collect::<Vec<_>>()
                                );
                            }
                            let value = self.context.control.operands.insert(FlowOperandFact {
                                descriptor_permission: if descriptor_loan.is_valid() {
                                    checked_trees::FlowDescriptorPermission::SuppliedOutput {
                                        loan: descriptor_loan,
                                    }
                                } else {
                                    Default::default()
                                },
                                type_reference: reference,
                                state_symbol: self.state.symbol,
                                statement_index: self.statement_index,
                                storage_snapshot: self.storage_snapshot,
                                referents,
                                retention,
                                ..Default::default()
                            });
                            if ::validation::trace_flags::check_sites() {
                                eprintln!(
                                    "CHECKSITE stored-call-value-created state={:?} statement={} target={:?} value={value:?} retention={retention:?} reference={reference:?} suppliers_known={suppliers_known} suppliers={:?} sources={:?}",
                                    self.state.symbol,
                                    self.statement_index,
                                    call.target_symbol,
                                    self.context
                                        .control
                                        .operand_retained_suppliers
                                        .span_or_empty(suppliers),
                                    effect
                                        .sources
                                        .iter()
                                        .map(|source| (
                                            source.backing.root,
                                            &source.backing.segments,
                                            &source.backing.recasts,
                                            &source.backing.representation,
                                            source.lifetime_access
                                        ))
                                        .collect::<Vec<_>>()
                                );
                            }
                            alternatives.push(CallStoredValue::Captured(value));
                        }
                        if effect.retains_incoming || effect.may_select || weak_destination {
                            alternatives.push(CallStoredValue::Incoming(self.storage_snapshot));
                        }
                        let value = match alternatives.len() {
                            0 => CallStoredValue::Unavailable,
                            1 => alternatives.pop().unwrap(),
                            _ => CallStoredValue::Alternatives(alternatives),
                        };
                        if let Some(previous) = values
                            .iter_mut()
                            .find(|value| value.destination == destination.place)
                        {
                            previous.value = CallStoredValue::Unavailable;
                        } else {
                            values.push(CallStore {
                                destination: destination.place,
                                value,
                            });
                        }
                    }
                }
                values
            }

            /*
                The helper's produced pointer keeps the permission of that exact
                output. A later copy can use it after the output variable changes.
                The resource consumer still authenticates normal completion; a pending
                output coordinate or equal address cannot authorize the pointer.
            */
            fn supplied_descriptor_permission(
                &self,
                call: &BorrowCallFact,
                origins: &[crate::flow::CapturedReferencePlace],
                reference: typed_trees::types::TypeReferenceHandle,
            ) -> Handle<checked_trees::BorrowLoanFact> {
                let [origin] = origins else {
                    return Handle::invalid();
                };
                let facts::PlaceRoot::InvocationOutput(identity) = origin.place.root else {
                    return Handle::invalid();
                };
                let Some(expected) = validation::reference_shell(self.program, reference) else {
                    return Handle::invalid();
                };
                let mut permissions = call.completed_reference_loans.handles().filter(|loan| {
                    let row = self.borrow.loans.get(*loan);
                    if row.lineage != checked_trees::BorrowLoanLineage::SuppliedOutput
                        || !row.established_by_call.is_valid()
                        || !self
                            .borrow
                            .incoming_references
                            .is_valid(row.incoming_reference)
                    {
                        return false;
                    }
                    let incoming = self.borrow.incoming_references.get(row.incoming_reference);
                    let path = self
                        .borrow
                        .owner_segments
                        .span_or_empty(incoming.owner_path);
                    let place_path = crate::borrow::owner_place_segments(path);
                    incoming.origin == (checked_trees::IncomingReferenceOrigin::Output { identity })
                        && place_path.len() == origin.place.segments.len()
                        && place_path
                            .iter()
                            .zip(&origin.place.segments)
                            .all(|(left, right)| {
                                validation::storage_path_relations::canonical_place_segments_equal(
                                    self.program,
                                    *left,
                                    *right,
                                )
                            })
                        && validation::project_type_reference_from_segments(
                            self.program,
                            incoming.carrier_type,
                            &place_path,
                        )
                        .and_then(|reference| validation::reference_shell(self.program, reference))
                        .is_some_and(|supplied| {
                            supplied.access == expected.access
                                && validation::storage_origin_types_match(
                                    self.program,
                                    supplied.referee,
                                    expected.referee,
                                )
                        })
                });
                let Some(permission) = permissions.next() else {
                    return Handle::invalid();
                };
                if permissions.next().is_some() {
                    Handle::invalid()
                } else {
                    permission
                }
            }
        }
    }

    use super::super::stored_values::append_preserved_binding;
    use super::super::stored_values::append_store;
    use super::super::stored_values::join;
    use super::{Execution, InvocationSite};
    use arena::{Handle, HandleSpan};
    use checked_trees::{BorrowCallFact, FlowOperandFact, FlowStorageSnapshot};
    use validation::CanonicalPlace;

    pub(super) struct CallStore {
        pub(super) destination: CanonicalPlace,
        pub(super) value: CallStoredValue,
    }

    pub(super) enum CallStoredValue {
        Unavailable,
        Incoming(Handle<FlowStorageSnapshot>),
        Captured(Handle<FlowOperandFact>),
        Alternatives(Vec<CallStoredValue>),
    }

    impl Execution<'_, '_, '_> {
        pub(super) fn capture_call_stores(
            &mut self,
            site: InvocationSite,
            call: &BorrowCallFact,
            inputs: HandleSpan<FlowOperandFact>,
            binding_writes: Option<&[CanonicalPlace]>,
        ) -> Vec<CallStore> {
            if matches!(site, InvocationSite::Transition(_)) {
                return Vec::new();
            }
            let mut inputs = inputs;
            let contract = if let Some(slot) =
                self.program.dynamic_slot_by_symbol(call.target_symbol)
            {
                if let Some((machine, state, selected)) = self.selected_store_target(call, inputs) {
                    inputs = selected;
                    validation::ReferenceSlotContract::CheckedBody { machine, state }
                } else {
                    let effects = self.context.call_frames.and_then(|frames| {
                        match frames
                            .received_selector_stored_reference_effects(self.state, slot.symbol)
                        {
                            Some(declared) => declared,
                            None => frames.dynamic_stored_reference_effects(slot.symbol),
                        }
                    });
                    let Some(effects) = effects else {
                        return Vec::new();
                    };
                    validation::ReferenceSlotContract::Opaque {
                        parameters: slot.parameters.as_slice(),
                        effects,
                    }
                }
            } else {
                let Some(contract) = self.context.call_frames.and_then(|frames| {
                    frames.reference_slot_contract(
                        self.machine,
                        self.state,
                        self.statement_index,
                        call.call_ordinal,
                        call.target_symbol,
                        call.authored_expression,
                    )
                }) else {
                    return Vec::new();
                };
                contract
            };
            let (parameters, effects) = match contract {
                validation::ReferenceSlotContract::CheckedBody { machine, state } => {
                    let parameters = self.program.state_parameters(state);
                    if inputs.len() != parameters.len() {
                        return Vec::new();
                    }
                    if self
                        .context
                        .stored_reference_effects(machine, state, &[])
                        .is_some_and(|effects| effects.is_empty())
                    {
                        return Vec::new();
                    }
                    let Some(indices) = self.captured_argument_indices(
                        parameters,
                        inputs
                            .handles()
                            .map(|handle| self.context.control.operands.get(handle)),
                    ) else {
                        return Vec::new();
                    };
                    let selectors = self.known_dynamic_selectors(parameters, inputs);
                    let effects = if selectors.is_empty() {
                        self.context
                            .stored_reference_effects(machine, state, &indices)
                    } else {
                        self.context.stored_reference_effects_for_selectors(
                            machine, state, &indices, &selectors,
                        )
                    };
                    let Some(effects) = effects else {
                        return Vec::new();
                    };
                    (parameters, effects)
                }
                validation::ReferenceSlotContract::Opaque {
                    parameters,
                    effects,
                } => (parameters, std::rc::Rc::new(effects)),
            };
            if inputs.len() != parameters.len() {
                return Vec::new();
            }
            self.capture_stored_suppliers(parameters, &effects, inputs, call, binding_writes)
        }

        /*
            Install the call's normal-return stores, then mark the storage version
            at which the call is complete: after its writes and its own stores.
        */
        pub(super) fn install_call_stores(
            &mut self,
            call: Handle<checked_trees::FlowCallFact>,
            stores: Vec<CallStore>,
        ) {
            for store in stores {
                self.storage_snapshot = self.install_call_stored_value(
                    self.storage_snapshot,
                    &store.destination,
                    store.value,
                );
            }
            if call.is_valid() {
                self.context.control.calls.get_mut(call).completed_storage = self.storage_snapshot;
            }
        }

        fn install_call_stored_value(
            &mut self,
            previous: Handle<FlowStorageSnapshot>,
            destination: &CanonicalPlace,
            value: CallStoredValue,
        ) -> Handle<FlowStorageSnapshot> {
            match value {
                CallStoredValue::Unavailable => append_store(
                    &mut self.context.control,
                    previous,
                    destination,
                    Handle::invalid(),
                    &[],
                ),
                CallStoredValue::Captured(value) => {
                    append_store(&mut self.context.control, previous, destination, value, &[])
                }
                CallStoredValue::Incoming(source) => append_preserved_binding(
                    &mut self.context.control,
                    previous,
                    destination,
                    source,
                ),
                CallStoredValue::Alternatives(values) => {
                    let predecessors = values
                        .into_iter()
                        .map(|value| self.install_call_stored_value(previous, destination, value))
                        .collect::<Vec<_>>();
                    join(&mut self.context.control, &predecessors)
                }
            }
        }
    }
}
mod transitions {
    use super::super::state_values::qualifications;
    use super::{TableTransition, TransitionTargetHandle, TransitionTargetNode};
    use crate::flow::expression::Execution;
    use crate::flow::expression::InvocationSite;
    use arena::HandleSpan;
    use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};

    impl Execution<'_, '_, '_> {
        /*
            Evaluate one selected arm's operands with the ordinary expression
            schedule, saving each completed value before evaluating the next operand.
        */
        pub(in crate::flow) fn transition_target(
            &mut self,
            transition: &TableTransition,
            target: TransitionTargetHandle,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            self.evaluation = checked_trees::FlowPermissionEvaluation::TransitionTarget(target);
            let operand_entry_constraints = *constraints;
            let previous_copies = self.context.active_completed_copies;
            self.context.active_completed_copies =
                self.capture_dispatch_payloads(transition, target, contexts, constraints);
            let mut returned_value = arena::Handle::invalid();
            let mut cannot_return_normally = false;
            let mut operands = Vec::new();
            let mut captured_operands = Vec::new();
            let mut values = Vec::new();
            let mut captured_qualifications = Vec::new();
            match self.program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { arguments, .. } => {
                    for (ordinal, argument) in self
                        .program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .enumerate()
                    {
                        let first_write = self.operand_writes.len();
                        operands.push((*argument, first_write));
                        let destination = self
                            .invocation_parameter_type(InvocationSite::Transition(target), ordinal);
                        captured_operands.push(
                            self.evaluated_operand_as(
                                *argument,
                                destination,
                                contexts,
                                constraints,
                            )
                            .capture,
                        );
                        let captured = captured_operands
                            .last()
                            .expect("completed transition argument");
                        let copied_values = if self.is_view_type(captured.type_reference)
                            && !matches!(
                                captured.completion,
                                checked_trees::FlowOperandCompletion::Copied { .. }
                            ) {
                            HandleSpan::empty()
                        } else {
                            self.context.active_completed_copies
                        };
                        captured_qualifications.extend(qualifications::capture_argument(
                            self.program,
                            self.semantic,
                            self.context,
                            self.machine,
                            self.state,
                            self.statement_index,
                            target,
                            ordinal,
                            *argument,
                            *contexts,
                            copied_values,
                        ));
                        values.push(super::super::state_values::capture_argument(
                            self.program,
                            self.semantic,
                            self.context,
                            self.machine,
                            self.state,
                            self.statement_index,
                            target,
                            ordinal,
                            *argument,
                            *contexts,
                            copied_values,
                        ));
                    }
                }
                TransitionTargetNode::Value(expression) => {
                    let completed = self.evaluated_operand_as(
                        *expression,
                        self.state.return_type,
                        contexts,
                        constraints,
                    );
                    cannot_return_normally = validation::expression_normal_completion_possible(
                        self.program,
                        *expression,
                    ) == Some(false);
                    if !cannot_return_normally {
                        returned_value = self.context.control.operands.append(completed.capture);
                    }
                }
                TransitionTargetNode::Terminal => {}
            }
            let bindings_stable = captured_qualifications
                .iter()
                .any(|captured| !captured.is_isolated())
                && self.context.call_frames.is_some_and(|frames| {
                    operands.iter().all(|(argument, _)| {
                        frames.expression_reference_bindings_are_stable(self.machine, *argument)
                    })
                });
            let argument_qualifications = qualifications::finish(
                self.context,
                *contexts,
                captured_qualifications,
                bindings_stable,
            );
            super::super::state_values::record_transition(
                self.program,
                self.semantic,
                self.context,
                self.machine,
                self.state,
                self.statement_index,
                transition,
                target,
                *contexts,
                &captured_operands,
                &values,
                argument_qualifications,
            );
            // Every selected edge publishes its outgoing frontier, including one
            // that cannot return (a diverging state call or a terminal external
            // completion such as process exit). The flag keeps it out of normal
            // return contracts; lowering still needs the edge's payload births.
            super::super::exits::append_outgoing_state_fact(
                self.context,
                self.machine.symbol,
                self.state.symbol,
                self.statement_index,
                target,
                operand_entry_constraints,
                *constraints,
                returned_value,
                self.storage_snapshot,
                cannot_return_normally,
                self.context.active_completed_copies,
            );
            let completed = self.invoke(
                InvocationSite::Transition(target),
                super::InvocationInputs {
                    operands: &captured_operands,
                },
                contexts,
                constraints,
            );
            self.install_call_stores(completed.call, completed.stores);
            self.context.active_completed_copies = previous_copies;
        }
    }
}
mod value_observations {

    /*
        Completed owned values publish observations at their evaluation cut.
        Construction rebases the same observations onto each installed child, while
        ordinary moves retain them on the completed operand before its former home
        expires. Only owned projections are snapshots: reference contents stay live
        and retain their separate backing, permission and invalidation obligations.
    */
    use super::{EvaluatedOperand, Execution};
    use arena::HandleSpan;
    use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
    use facts::{
        Fact, FactOrigin, FactPayload, FactPlace, PlaceSegment, ProgramPoint, ScalarValue,
    };
    use typed_trees::expression::ExpressionHandle;

    #[derive(Clone, PartialEq, Eq)]
    pub(super) enum OwnedValueObservation {
        Scalar(ScalarValue),
        IntegerBounds(facts::IntegerRange),
        Case(symbols::SymbolHandle),
        Capture {
            source: facts::PlaceHandle,
            point: ProgramPoint,
        },
    }

    impl OwnedValueObservation {
        /*
            Keep what both completed values establish. Different integer values
            still establish the range containing both; other observations must agree.
        */
        pub(super) fn common_consequence(&self, other: &Self) -> Option<Self> {
            if self == other {
                return Some(self.clone());
            }
            let left = self.integer_bounds()?;
            let right = other.integer_bounds()?;
            Some(Self::IntegerBounds(facts::IntegerRange {
                minimum: left.minimum.min(right.minimum),
                maximum: left.maximum.max(right.maximum),
            }))
        }

        fn integer_bounds(&self) -> Option<facts::IntegerRange> {
            match self {
                Self::Scalar(ScalarValue::Integer(value)) => Some(facts::IntegerRange {
                    minimum: value.clone(),
                    maximum: value.clone(),
                }),
                Self::IntegerBounds(bounds) => Some(bounds.clone()),
                _ => None,
            }
        }
    }

    impl Execution<'_, '_, '_> {
        /*
            A value read from storage has its own classifier even after an affine
            move retires that home. Publish before recording reference-leaf owners;
            they must name this completed value rather than a later source reread.
        */
        pub(super) fn publish_completed_owned_value(
            &mut self,
            evaluated: &mut EvaluatedOperand,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let expression = evaluated.capture.expression;
            let Some(source) = self.completed_value_place(expression) else {
                return;
            };
            if source.root == facts::PlaceRoot::Expression(expression) && source.segments.is_empty()
            {
                return;
            }
            self.publish_owned_capture(expression, evaluated, contexts, constraints);
        }

        /// One completed producer supplies every saved namespace. Memberships do
        /// not depend on whether the value also has a scalar or tag observation.
        pub(super) fn publish_owned_capture(
            &mut self,
            parent: ExpressionHandle,
            evaluated: &mut EvaluatedOperand,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            if evaluated.capture.state_symbol != self.state.symbol
                || evaluated.capture.statement_index != self.statement_index
                || !self.program.expression_table.expression_is_valid(parent)
                || !evaluated.capture.type_reference.is_valid()
                || validation::reference_shell(self.program, evaluated.capture.type_reference)
                    .is_some()
            {
                return;
            }
            let values = self.captured_owned_value_observations(
                &[],
                evaluated.capture.type_reference,
                evaluated,
            );
            let previous = self
                .context
                .contexts
                .constraint_refs
                .span_or_empty(*constraints)
                .len();
            self.publish_owned_value_observations(parent, values, contexts, constraints);
            self.append_completed_qualifications(
                parent,
                &[],
                evaluated.capture.type_reference,
                evaluated,
                contexts,
                constraints,
            );
            // capture_operand may already have withdrawn source facts invalidated
            // by child effects. Extend that filtered snapshot only with the newly
            // published value facts, never restore the ambient unfiltered prefix.
            let published = *constraints;
            for handle in published.handles().skip(previous) {
                let kind = self.context.contexts.constraint_refs.get(handle).kind;
                crate::flow::append_constraint_ref(
                    &mut self.context.contexts.constraint_refs,
                    &mut evaluated.capture.constraints,
                    kind,
                );
            }
        }

        pub(super) fn append_owned_value_observations(
            &mut self,
            parent: ExpressionHandle,
            destination: &[PlaceSegment],
            type_reference: typed_trees::types::TypeReferenceHandle,
            evaluated: &EvaluatedOperand,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let mut values =
                self.captured_owned_value_observations(destination, type_reference, evaluated);
            if validation::has_contents(
                self.program,
                type_reference,
                &[],
                validation::ContentsRequirement::BorrowFree,
            ) && !values.iter().any(|(path, value)| {
                path == destination
                    && matches!(
                        value,
                        OwnedValueObservation::Scalar(_) | OwnedValueObservation::Capture { .. }
                    )
            }) {
                let source = self.semantic.append_expression_place(parent);
                for segment in destination {
                    self.semantic.push_place_segment(source, *segment);
                }
                values.push((
                    destination.to_vec(),
                    OwnedValueObservation::Capture {
                        source,
                        point: ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                        },
                    },
                ));
            }
            self.publish_owned_value_observations(parent, values, contexts, constraints);
            self.append_completed_qualifications(
                parent,
                destination,
                type_reference,
                evaluated,
                contexts,
                constraints,
            );
            self.append_constructed_child_qualifications(
                parent,
                destination,
                type_reference,
                evaluated,
                contexts,
                constraints,
            );
        }

        /*
            The construction checker independently forms each declared child domain
            against its earlier completed capture. Publish that pending judgment on
            the parent field, never into the child's own proof context: otherwise a
            receiving annotation could discharge its own construction obligation.
            Existing views and storage reads only transport their recorded facts.
        */
        fn append_constructed_child_qualifications(
            &mut self,
            parent: ExpressionHandle,
            destination: &[PlaceSegment],
            type_reference: typed_trees::types::TypeReferenceHandle,
            evaluated: &EvaluatedOperand,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            if !matches!(
                evaluated.capture.completion,
                checked_trees::FlowOperandCompletion::Constructed(_)
            ) || validation::reference_shell(self.program, type_reference).is_some()
            {
                return;
            }
            let domains = validation::field_domains::domain_constraint_identities(
                self.program,
                type_reference,
            );
            for (domain_symbol, semantic_domain) in domains {
                if validation::field_domains::domain_requires_provenance(
                    self.program,
                    domain_symbol,
                ) || !crate::flow::domain::qualification_is_content_independent(
                    self.program,
                    domain_symbol,
                ) {
                    continue;
                }
                let Some(dependency) = self.domains.dependency_fact(domain_symbol) else {
                    continue;
                };
                if !self.domains.dependency_paths(dependency).all(|path| {
                    (0..=path.len()).all(|length| {
                        validation::project_type_reference_from_segments(
                            self.program,
                            type_reference,
                            &path[..length],
                        )
                        .is_some_and(|reference| {
                            !crate::flow::operand_binds_live_view(self.program, reference)
                        })
                    })
                }) {
                    continue;
                }
                let place = crate::published::semantic::places::append_place(
                    self.semantic,
                    facts::PlaceRoot::Expression(parent),
                    destination,
                );
                let context = crate::published::semantic::facts::append_observation_context(
                    self.semantic,
                    Fact {
                        place: FactPlace::Place(place),
                        point: ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                        },
                        origin: FactOrigin::StatementTransfer,
                        evidence: crate::published::facts::checked_validation_evidence(
                            self.state.symbol,
                        ),
                        payload: FactPayload::DomainMembership {
                            value: parent,
                            domain: HandleSpan::empty(),
                            domain_symbol,
                            semantic_domain,
                        },
                    },
                    &[FactPlace::Place(place)],
                );
                self.context
                    .append_semantic_context(contexts, constraints, context);
            }
        }

        /*
            Rebase only the completed owned value's existing qualifications. A
            construction stores these at its child path; a selection meets them at
            its receiving root. Both retain the same source and authority rules.
        */
        pub(super) fn append_completed_qualifications(
            &mut self,
            parent: ExpressionHandle,
            destination: &[PlaceSegment],
            type_reference: typed_trees::types::TypeReferenceHandle,
            evaluated: &EvaluatedOperand,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let Some(source) = self.context.canonical_place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                evaluated.capture.expression,
            ) else {
                return;
            };
            let source = crate::published::semantic::places::append_place(
                self.semantic,
                source.root,
                &source.segments,
            );
            let destination = crate::published::semantic::places::append_place(
                self.semantic,
                facts::PlaceRoot::Expression(parent),
                destination,
            );
            let point = ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            };
            // A retained operand can outlive source writes in a later sibling or
            // pattern. Its immutable completed constraint roster owns this copy;
            // the caller's current ambient contexts are only the output frontier.
            let captured_contexts = self
                .context
                .contexts
                .constraint_refs
                .span_or_empty(evaluated.capture.constraints)
                .iter()
                .filter_map(|reference| match reference.kind {
                    FlowConstraintKind::SemanticContext { context } => Some(context),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut references = HandleSpan::empty();
            crate::flow::transfers::append_owned_qualification_transfer(
                self.program,
                self.semantic,
                &captured_contexts,
                evaluated.capture.expression,
                source,
                destination,
                type_reference,
                point,
                &mut references,
            );
            for reference in references.handles() {
                let fact = self.semantic.refs.get(reference).fact;
                let mut single = HandleSpan::empty();
                self.semantic.append_ref(&mut single, fact);
                let context = self.semantic.append_context(point, single);
                self.context
                    .append_semantic_context(contexts, constraints, context);
            }
        }

        /*
            Snapshot only the completed owned value's observations. Match joins keep
            their common consequences; no arm or initializer is replayed.
        */
        pub(super) fn captured_owned_value_observations(
            &mut self,
            destination: &[PlaceSegment],
            type_reference: typed_trees::types::TypeReferenceHandle,
            evaluated: &EvaluatedOperand,
        ) -> Vec<(Vec<PlaceSegment>, OwnedValueObservation)> {
            if !type_reference.is_valid()
                || !validation::has_contents(
                    self.program,
                    type_reference,
                    &[],
                    validation::ContentsRequirement::StoredValue,
                )
                || validation::reference_shell(self.program, type_reference).is_some()
            {
                return Vec::new();
            }
            let captured_contexts = self
                .context
                .contexts
                .constraint_refs
                .span_or_empty(evaluated.capture.constraints)
                .iter()
                .filter_map(|reference| {
                    if let FlowConstraintKind::SemanticContext { context } = reference.kind {
                        Some(context)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let mut values = Vec::new();
            let primitive = self.program.primitive_type_reference(type_reference);
            let completed_value = primitive.and_then(|_| {
                let source = self.completed_value_place(evaluated.capture.expression)?;
                if !validation::value_has_builtin_coordinates(
                    self.program,
                    self.machine,
                    self.state,
                    evaluated.capture.expression,
                ) {
                    return None;
                }
                crate::values::scalar_value_at_place(
                    self.program,
                    self.semantic,
                    captured_contexts
                        .iter()
                        .map(|context| self.semantic.contexts.get(*context)),
                    &source,
                )
            });
            let scalar = completed_value
                .or_else(|| evaluated.boolean.map(ScalarValue::Boolean))
                .or_else(|| {
                    let primitive = primitive?;
                    let expression = evaluated.capture.expression;
                    if !validation::has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) {
                        return None;
                    }
                    let expression = crate::values::lower_unit_scalar_argument(
                        self.program,
                        self.context.operators,
                        self.state,
                        self.statement_index,
                        expression,
                        primitive,
                    )?;
                    let symbols = crate::values::unit_scalar_binding_symbols(
                        self.program,
                        self.state,
                        self.statement_index,
                    )?;
                    crate::values::evaluate_checked_scalar(
                        &expression,
                        &mut crate::values::PlaceScalarValues {
                            program: self.program,
                            parameters: self.program.state_parameters(self.state),
                            symbols: &symbols,
                            value_at_place: |place: &validation::CanonicalPlace, observation| {
                                let place = self.context.completed_read_place(
                                    self.program,
                                    self.state.symbol,
                                    self.statement_index,
                                    observation,
                                    place,
                                )?;
                                crate::values::scalar_value_at_place(
                                    self.program,
                                    self.semantic,
                                    captured_contexts
                                        .iter()
                                        .map(|context| self.semantic.contexts.get(*context)),
                                    &place,
                                )
                            },
                        },
                    )
                });
            if let Some(value) = scalar {
                values.push((destination.to_vec(), OwnedValueObservation::Scalar(value)));
            }
            if values.is_empty()
                && let Some(primitive) = primitive
                && let Some(carrier) = validation::carrier_integer_range(primitive)
                && validation::value_has_builtin_coordinates(
                    self.program,
                    self.machine,
                    self.state,
                    evaluated.capture.expression,
                )
                && let Some(source) = self.completed_value_place(evaluated.capture.expression)
                && let Some(bounds) = crate::values::CurrentIntegerBounds::new(
                    self.program,
                    self.context.operators,
                    self.semantic,
                    &captured_contexts,
                    self.state.symbol,
                    ProgramPoint::Statement {
                        machine_symbol: self.machine.symbol,
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                    },
                )
                .at_place(&source, primitive)
                && bounds != carrier
            {
                values.push((
                    destination.to_vec(),
                    OwnedValueObservation::IntegerBounds(bounds),
                ));
            }
            let is_aggregate = primitive.is_none();
            if is_aggregate
                && validation::value_has_builtin_coordinates(
                    self.program,
                    self.machine,
                    self.state,
                    evaluated.capture.expression,
                )
                && let Some(source) = self.completed_value_place(evaluated.capture.expression)
            {
                let mut case_evidence: Option<Option<crate::flow::CaseEvidence<'_>>> = None;
                let mut boolean_values = crate::checks::contracts::BooleanValueQueries::new(
                    self.program,
                    self.context.operators,
                    self.semantic,
                    &captured_contexts,
                    ProgramPoint::Statement {
                        machine_symbol: self.machine.symbol,
                        state_symbol: self.state.symbol,
                        statement_index: self.statement_index,
                    },
                );
                let mut assignment_values = None;
                let mut seen_paths = values
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect::<std::collections::HashSet<Vec<PlaceSegment>>>();
                for context in &captured_contexts {
                    for fact in self
                        .semantic
                        .context_view(self.semantic.contexts.get(*context))
                        .facts()
                    {
                        let Some(place) = fact.place.as_place() else {
                            continue;
                        };
                        let Some(place) = crate::flow::canonical_place_from_semantic_place(
                            self.program,
                            self.semantic,
                            self.semantic.places.get(place),
                        ) else {
                            continue;
                        };
                        if place.root != source.root {
                            continue;
                        }
                        let Some(relative) =
                            place.segments.strip_prefix(source.segments.as_slice())
                        else {
                            continue;
                        };
                        if !relative
                            .iter()
                            .all(crate::flow::place_segment_has_static_identity)
                        {
                            continue;
                        }
                        let Some(member_type) = validation::owned_projection_type_reference(
                            self.program,
                            type_reference,
                            relative,
                        ) else {
                            continue;
                        };
                        if validation::reference_shell(self.program, member_type).is_some()
                            || (!matches!(fact.payload, FactPayload::CaseValue { value: true, .. })
                                && !validation::has_contents(
                                    self.program,
                                    member_type,
                                    &[],
                                    validation::ContentsRequirement::BorrowFree,
                                ))
                        {
                            continue;
                        }
                        let mut path = destination.to_vec();
                        path.extend_from_slice(relative);
                        if seen_paths.contains(&path) {
                            continue;
                        }
                        if place
                            .segments
                            .iter()
                            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
                            && !case_evidence
                                .get_or_insert_with(|| {
                                    crate::flow::CaseEvidence::prepare(
                                        self.program,
                                        self.semantic,
                                        &captured_contexts,
                                        self.machine.symbol,
                                        self.state.symbol,
                                        self.statement_index,
                                    )
                                })
                                .as_ref()
                                .is_some_and(|evidence| evidence.cases_are_selected(&place))
                        {
                            continue;
                        }
                        let value = match fact.payload {
                            FactPayload::AssignedIntegerBounds { bounds } => {
                                Some(OwnedValueObservation::IntegerBounds(
                                    self.semantic.integer_ranges.get(bounds).clone(),
                                ))
                            }
                            FactPayload::CaseValue {
                                variant,
                                value: true,
                            } => Some(OwnedValueObservation::Case(variant)),
                            FactPayload::AssignedValueCapture { source, point } => {
                                Some(OwnedValueObservation::Capture { source, point })
                            }
                            _ => (boolean_values
                                .at_place(&place)
                                .map(ScalarValue::Boolean)
                                .or_else(|| {
                                    assignment_values
                                        .get_or_insert_with(|| {
                                            crate::values::AssignmentObservationBatch::prepare_for_root(
                                                self.program,
                                                self.semantic,
                                                captured_contexts.iter().map(|context| {
                                                    self.semantic.contexts.get(*context)
                                                }),
                                                source.root,
                                            )
                                        })
                                        .scalar_value(&place)
                                }))
                            .map(OwnedValueObservation::Scalar),
                        };
                        let Some(value) = value else {
                            continue;
                        };
                        seen_paths.insert(path.clone());
                        values.push((path, value));
                    }
                }
            }
            values
        }

        pub(super) fn publish_owned_value_observations(
            &mut self,
            parent: ExpressionHandle,
            values: Vec<(Vec<PlaceSegment>, OwnedValueObservation)>,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            for (path, value) in values {
                let payload = match value {
                    OwnedValueObservation::Scalar(value) => FactPayload::AssignedScalarValue {
                        value: self.semantic.scalar_values.append(value),
                    },
                    OwnedValueObservation::IntegerBounds(bounds) => {
                        FactPayload::AssignedIntegerBounds {
                            bounds: self.semantic.integer_ranges.append(bounds),
                        }
                    }
                    OwnedValueObservation::Case(variant) => FactPayload::CaseValue {
                        variant,
                        value: true,
                    },
                    OwnedValueObservation::Capture { source, point } => {
                        FactPayload::AssignedValueCapture { source, point }
                    }
                };
                self.append_value_observation(parent, &path, payload, contexts, constraints);
            }
        }

        /*
            Each observation has its own lifetime. A nested case does not lose its
            tag merely because a sibling scalar or its own payload was replaced.
        */
        pub(super) fn append_value_observation(
            &mut self,
            parent: ExpressionHandle,
            path: &[PlaceSegment],
            payload: FactPayload,
            contexts: &mut HandleSpan<FlowSemanticContextRef>,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let point = ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.state.symbol,
                statement_index: self.statement_index,
            };
            let place = self.semantic.append_expression_place(parent);
            for segment in path {
                self.semantic.push_place_segment(place, *segment);
            }
            let mut references = HandleSpan::empty();
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: crate::published::facts::propagated_evidence(self.state.symbol),
                payload,
            });
            self.semantic.append_ref(&mut references, fact);
            let context = self.semantic.append_context(point, references);
            self.context
                .append_semantic_context(contexts, constraints, context);
        }
    }
}
mod view_footprints {
    /*
        Completed storage geometry stays separate from lifetime and exact identity.
        Checked bodies retain complete possible coordinates for interference and
        typed field projection. A representation view instead keeps its original
        backing enclosure: its new field symbols cannot name the source layout.
        Resident views borrow content, excluding their Extent descriptor.
    */
    use super::Execution;
    use crate::flow::expressions;
    use arena::{Handle, HandleSpan};
    use checked_trees::{FlowCallFact, FlowOperandFact, FlowOperandReferent, FlowOperandRetention};
    use facts::PlaceSegment;
    use language_semantics::MachineSupplyMode;
    use typed_trees::expression::{ExpressionHandle, TableCallExpression};
    use validation::CanonicalPlace;
    use validation::ReferenceFormationInterpretation;
    use validation::call_target_state;
    use validation::completed_storage::direct_referent_places;
    use validation::completed_storage::projectable_reference_places;
    use validation::owned_projection_type_reference;
    use validation::reference_formation_source;
    use validation::reference_shell;
    use validation::storage_origin_types_match;

    impl Execution<'_, '_, '_> {
        /*
            Owned projections preserve every possible backing without claiming an
            exact address. A dynamic selector contributes an element dimension, not
            an expression that a later write may evaluate again.
        */
        pub(super) fn project_possible_storage(
            &mut self,
            value: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<HandleSpan<FlowOperandReferent>> {
            let backing = self.completed_backing_places(value)?;
            if backing.is_empty() {
                return None;
            }
            let path = path
                .iter()
                .map(|segment| match segment {
                    PlaceSegment::Index { .. } => PlaceSegment::Index {
                        expression: typed_trees::expression::ExpressionHandle::invalid(),
                    },
                    segment => *segment,
                })
                .collect::<Vec<_>>();
            let projected = backing
                .into_iter()
                .map(|place| crate::flow::project_captured_view_place(place, &path))
                .collect::<Option<Vec<_>>>()?;
            Some(self.append_referent_places(&projected))
        }

        pub(super) fn view_enclosure(
            &self,
            profile: Handle<FlowOperandRetention>,
        ) -> Handle<FlowOperandFact> {
            if self.context.control.operand_retentions.is_valid(profile) {
                self.context
                    .control
                    .operand_retentions
                    .get(profile)
                    .enclosing_storage
            } else {
                Handle::invalid()
            }
        }

        pub(super) fn reinterpreted_view_retention(
            &mut self,
            source: &FlowOperandFact,
            lifetime: Handle<FlowOperandRetention>,
        ) -> Handle<FlowOperandRetention> {
            if !self.context.control.operand_retentions.is_valid(lifetime) {
                return lifetime;
            }
            let profile = *self.context.control.operand_retentions.get(lifetime);
            let enclosing_storage = self.context.control.operands.insert(*source);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    enclosing_storage,
                    possible_storage: arena::HandleSpan::empty(),
                    ..profile
                })
        }

        pub(super) fn capture_formation_enclosure(
            &mut self,
            expression: ExpressionHandle,
            call: &TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            retention: &mut Option<Handle<FlowOperandRetention>>,
        ) {
            let Some(formation) = reference_formation_source(self.program, expression) else {
                return;
            };
            if formation.interpretation != ReferenceFormationInterpretation::Access {
                return;
            }
            let Some([(_, source)]) = inputs.get(usize::from(call.receiver.is_valid())..) else {
                return;
            };
            if let Some(lifetime) = *retention {
                *retention = Some(self.reinterpreted_view_retention(source, lifetime));
            }
        }
        /*
            A complete conservative footprint is separate from the exact pointer
            roster. Preserve it with the descriptor through stores and forwarding.
        */
        pub(super) fn retain_possible_storage(
            &mut self,
            value: &mut FlowOperandFact,
            places: &[CanonicalPlace],
        ) {
            if places.is_empty()
                || !self
                    .context
                    .control
                    .operand_retentions
                    .is_valid(value.retention)
            {
                return;
            }
            let possible_storage = self.append_referent_places(places);
            let profile = *self.context.control.operand_retentions.get(value.retention);
            value.retention =
                self.context
                    .control
                    .operand_retentions
                    .insert(FlowOperandRetention {
                        possible_storage,
                        ..profile
                    });
        }

        /*
            A joined view is projectable only if every retained alternative is.
            Missing reference metadata or an enclosure supplies no typed field path.
        */
        pub(super) fn backing_allows_projection(&self, value: &FlowOperandFact) -> bool {
            if !value.retention.is_valid() {
                return !self.is_view_type(value.type_reference);
            }
            retained_backing_allows_projection(&self.context.control, value.retention)
        }

        pub(super) fn completed_backing_places(
            &mut self,
            value: &FlowOperandFact,
        ) -> Option<Vec<CanonicalPlace>> {
            if !self.backing_allows_projection(value) {
                return None;
            }
            let places = if !value.referents.is_empty() {
                direct_referent_places(&self.context.control, value.referents)?
            } else {
                let operand = self.context.control.operands.insert(*value);
                projectable_reference_places(self.program, &self.context.control, operand, &[])?
            };
            Some(
                places
                    .into_iter()
                    .map(|place| CanonicalPlace {
                        root: place.root,
                        segments: place.segments,
                    })
                    .collect(),
            )
        }

        pub(super) fn capture_invocation_backing(
            &mut self,
            invocation: Handle<FlowCallFact>,
            value: &FlowOperandFact,
        ) -> Option<Vec<CanonicalPlace>> {
            if !self.context.control.calls.is_valid(invocation) {
                return None;
            }
            let call = self.context.control.calls.get(invocation).clone();
            if call.authored_expression != value.expression
                || call.statement_index != self.statement_index
            {
                return None;
            }
            let source = self.program.expression_table.call_node(value.expression)?;
            if !source.carries_only_positional_arguments() || !source.selects_only_nominal_route() {
                return None;
            }
            let output = reference_shell(self.program, value.type_reference)?;
            let state = call_target_state(self.program, call.target_symbol)?;
            let machine = self.program.machine_holding_state(state.symbol)?;
            if !machine.body_is_present
                || machine.supply_mode != MachineSupplyMode::CheckedBody
                || machine.attached_data.is_some()
            {
                return None;
            }
            let parameters = self.program.state_parameters(state);
            let actuals = self
                .context
                .control
                .operands
                .span_or_empty(call.operands)
                .to_vec();
            if actuals.len() != parameters.len() {
                return None;
            }
            let origins = self
                .context
                .call_frames?
                .reference_result_leaf_origins(state.symbol)?;
            let mut backing = Vec::new();
            for origin in origins {
                let source = origin.source;
                if !origin.result_path.is_empty()
                    || source.is_image_storage
                    || source.invocation_output.is_some()
                    || !source.recasts.is_empty()
                    || source.representation.is_some()
                {
                    return None;
                }
                let argument_index = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == source.root)?;
                let actual = actuals.get(argument_index)?;
                let input = reference_shell(self.program, actual.type_reference)?;
                let mut segments = source
                    .segments
                    .into_iter()
                    .map(|segment| {
                        segment.erase_runtime_selector(|expression| {
                            expressions(self.program).range_node(expression).is_some()
                        })
                    })
                    .collect();
                crate::flow::compose_captured_view_segments(&mut segments)?;
                let selected =
                    owned_projection_type_reference(self.program, input.referee, &segments)?;
                if !storage_origin_types_match(self.program, selected, output.referee) {
                    return None;
                }
                let addresses = self.completed_backing_places(actual)?;
                if addresses.is_empty() {
                    return None;
                }
                for address in addresses {
                    let place = crate::flow::project_captured_view_place(address, &segments)?;
                    crate::shared::collections::push_unique(&mut backing, place);
                }
            }
            (!backing.is_empty()).then_some(backing)
        }
    }

    fn retained_backing_allows_projection(
        control: &checked_trees::FlowControlFacts,
        retention: Handle<FlowOperandRetention>,
    ) -> bool {
        let profiles = &control.operand_retentions;
        if !profiles.is_valid(retention) {
            return false;
        }
        let root = profiles.get(retention);
        if root.enclosing_storage.is_valid() || !root.enclosing_regions.is_empty() {
            return false;
        }
        if root.alternatives.is_empty() {
            return true;
        }
        let mut remaining = profiles.len();
        let mut pending = vec![(retention, false)];
        let mut active = std::collections::HashSet::new();
        let mut completed = std::collections::HashSet::new();
        while let Some((handle, leaving)) = pending.pop() {
            if leaving {
                active.remove(&handle);
                completed.insert(handle);
                continue;
            }
            if completed.contains(&handle) {
                continue;
            }
            if !profiles.is_valid(handle) || !active.insert(handle) {
                return false;
            }
            let Some(next) = remaining.checked_sub(1) else {
                return false;
            };
            remaining = next;
            let profile = profiles.get(handle);
            if profile.enclosing_storage.is_valid() || !profile.enclosing_regions.is_empty() {
                return false;
            }
            pending.push((handle, true));
            if !profile.alternatives.is_empty() {
                if profiles.span(profile.alternatives).is_none() {
                    return false;
                }
                pending.extend(profile.alternatives.handles().map(|handle| (handle, false)));
            }
        }
        true
    }

    #[cfg(test)]
    mod backing_profiles_tests {
        use super::retained_backing_allows_projection;
        use arena::Handle;
        use checked_trees::{FlowControlFacts, FlowOperandRetention};

        #[test]
        fn backing_profiles_keep_enclosures_after_joins() {
            let mut control = FlowControlFacts::default();
            let leaf = control
                .operand_retentions
                .insert_many([FlowOperandRetention::default()]);
            let shared = control.operand_retentions.insert_many([
                FlowOperandRetention {
                    alternatives: leaf,
                    ..Default::default()
                },
                FlowOperandRetention {
                    alternatives: leaf,
                    ..Default::default()
                },
            ]);
            let joined = control.operand_retentions.insert(FlowOperandRetention {
                alternatives: shared,
                ..Default::default()
            });
            assert!(retained_backing_allows_projection(&control, joined));
            control
                .operand_retentions
                .get_mut(leaf.handles().next().unwrap())
                .enclosing_storage = Handle::from_arena_index(1);
            assert!(!retained_backing_allows_projection(&control, joined));
        }

        #[test]
        fn backing_profiles_reject_cycles_and_missing_rows() {
            let mut control = FlowControlFacts::default();
            let cycle = control
                .operand_retentions
                .insert_many([FlowOperandRetention::default()]);
            let root = cycle.handles().next().unwrap();
            control.operand_retentions.get_mut(root).alternatives = cycle;
            assert!(!retained_backing_allows_projection(&control, root));
            assert!(!retained_backing_allows_projection(
                &control,
                Handle::from_arena_index(999)
            ));
        }
    }
}
mod view_referents {
    /*
        Addresses are formed from completed operands, not the final selector
        variables. A bound can change while a later bound evaluates; each coordinate
        therefore reads its own capture. Stored descriptors retain these addresses
        independently of later writes to their backing contents.
    */
    use super::super::CapturedReferencePlace;
    use super::super::observations::completed_copy_read_place;
    use super::super::observations::descriptor_observation_namespace;
    use super::super::operand_values::captured_operand_reference_places_with_formation;
    use super::super::operand_values::finite_projection_paths;
    use super::super::project_captured_view_place;
    use super::super::reference_places::ReferenceResultSourceKind;
    use super::super::reference_places::call_result_captured_sources;
    use super::super::reference_places::call_result_leaf_sources;
    use super::super::stored_values::StoredOperand;
    use super::super::stored_views::capture_at_snapshot;
    use super::super::stored_views::reference_places_at_snapshot;
    use super::reference_projection::ReferenceProjection;
    use crate::published::places::place_at;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::TypeReferenceHandle;
    use validation::borrow_contracts::borrow_carrying_owner_paths;
    use validation::completed_storage::captured_retention_profiles_at_path;
    use validation::reference_result_custody::loan_place_segments;
    /*
        Ephemeral results of the shared immutable completed-storage query.
        This is neither a second store inventory nor permission to load a pointer.
    */
    pub(super) enum StoredReferenceContents {
        Unknown,
        Incoming,
        /*
            A literal or other immutable image. It has no caller-local address, and
            that absence is the complete backing, not an unresolved pointer.
        */
        Image,
        Captured {
            referents: HandleSpan<FlowOperandReferent>,
        },
    }
    use super::Execution;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        FlowCallFact, FlowCapturedReferenceLeaf as ReferenceLeaf, FlowCaseAccessFact,
        FlowConstraintKind, FlowConstraintRef, FlowOperandAddressProjection, FlowOperandCompletion,
        FlowOperandFact, FlowOperandReferent, FlowOperandRetainedSupplier, FlowReferentFormation,
        FlowReferentFormationEvidence as FormationEvidence,
    };
    use facts::{
        PlaceRoot, PlaceSegment, ScalarValue, payload_variant_for_field,
        project_captured_view_segments,
    };
    use typed_trees::expression::{ExpressionHandle, TableCallExpression};
    use validation::{
        CanonicalPlace, canonical_place_type_reference, captured_integer_value,
        declared_place_type_raw, effective_member_symbol, has_builtin_bound_expression_meaning,
        indexed_operation_has_builtin_meaning, owned_projection_type_reference, reference_shell,
    };

    impl Execution<'_, '_, '_> {
        /*
            A returned value acquires its permission at this exact completion,
            before an enclosing call can consume it. Its ordinary lifetime ledger
            retains every reference and input lifetime when the result is saved.
        */
        pub(super) fn retain_invocation_permissions(
            &mut self,
            invocation: Handle<FlowCallFact>,
            capture: &FlowOperandFact,
        ) {
            if !self.context.control.calls.is_valid(invocation) {
                return;
            }
            let actual = self.context.control.calls.get(invocation);
            let Some(call) = self.borrow.call_at(
                self.machine.symbol,
                self.state.symbol,
                actual.statement_index,
                actual.call_ordinal,
            ) else {
                return;
            };
            let source = self.borrow.calls.get(call);
            if actual.authored_expression != capture.expression
                || source.authored_expression != capture.expression
                || source.target_symbol != actual.target_symbol
                || actual.statement_index != capture.statement_index
            {
                return;
            }
            let loans = source
                .completed_reference_loans
                .handles()
                .filter(|loan| {
                    let row = self.borrow.loans.get(*loan);
                    self.borrow.is_call_result_loan(row)
                        && row.established_by_call == call
                        && row.formation_expression == capture.expression
                        && self
                            .context
                            .contexts
                            .constraint_refs
                            .span_or_empty(capture.constraints)
                            .iter()
                            .any(|reference| {
                                matches!(reference.kind,
                            FlowConstraintKind::BorrowLoan { loan: active } if active == *loan)
                            })
                })
                .collect::<Vec<_>>();
            if loans.is_empty() {
                return;
            }
            let formation_value = self.context.control.operands.append(*capture);
            for loan in loans {
                self.context.borrow_lifetimes.activations.append(
                    checked_trees::FlowBorrowActivationFact {
                        source: checked_trees::FlowInvalidationSource::Statement {
                            statement_index: self.statement_index,
                        },
                        loan,
                        formation_value,
                    },
                );
            }
        }
        /*
            Bind a prepared pointer resource to the actual completed slot read.
            The access certificate precedes activation: this child permission can
            never establish the read that supplies it. Owned copies and selections
            retain their earlier producers rather than receiving a new namespace.
        */
        pub(super) fn retain_descriptor_permission(
            &mut self,
            capture: &mut FlowOperandFact,
            constraints: &mut HandleSpan<FlowConstraintRef>,
        ) {
            let Some((_, state)) = self
                .borrow
                .first_state_borrow(self.machine.symbol, self.state.symbol)
            else {
                return;
            };
            let mut resources = self.borrow.state_loans(state).filter(|(_, loan)| {
                loan.statement_index == self.statement_index
                    && loan.formation_expression == capture.expression
                    && crate::borrow::is_descriptor_load_observation(self.borrow, loan)
            });
            let Some((handle, loan)) = resources.next() else {
                return;
            };
            if resources.next().is_some() {
                return;
            }
            if capture.descriptor_permission != checked_trees::FlowDescriptorPermission::Unavailable
                || !matches!(
                    capture.completion,
                    checked_trees::FlowOperandCompletion::Observation
                )
                || validation::reference_shell(self.program, capture.type_reference).is_none()
                || !self
                    .context
                    .control
                    .case_accesses
                    .is_valid(capture.case_access)
            {
                return;
            }
            let access = self.context.control.case_accesses.get(capture.case_access);
            if access.machine_symbol != self.machine.symbol
                || access.state_symbol != self.state.symbol
                || access.statement_index != self.statement_index
                || access.expression != capture.expression
                || access.evaluation != self.evaluation
                || access.places.is_empty()
            {
                return;
            }
            if !self.context.control.operands.is_valid(capture.backing) {
                return;
            }
            let receiver = self.context.control.operands.get(capture.backing);
            let Some(source) = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                capture.expression,
            ) else {
                return;
            };
            let Some(parent) = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                receiver.expression,
            ) else {
                return;
            };
            if source.root != parent.root || !source.segments.starts_with(&parent.segments) {
                return;
            }
            let projection = &source.segments[parent.segments.len()..];
            let incoming = validation::completed_storage::reference_sources_at_path(
                self.program,
                &self.context.control,
                capture.backing,
                projection,
                capture.storage_snapshot,
            );
            fn complete_incoming(
                program: &dyn validation::CheckingInput,
                state: &typed_trees::state::State,
                source: &validation::completed_storage::StoredOperand,
            ) -> bool {
                use validation::completed_storage::StoredOperand;
                match source {
                    StoredOperand::Incoming(place) => {
                        place.root.as_symbol().is_some_and(|symbol| {
                            program.state_parameter_by_symbol(state, symbol).is_some()
                        })
                    }
                    StoredOperand::Alternatives(alternatives) => {
                        !alternatives.is_empty()
                            && alternatives
                                .iter()
                                .all(|source| complete_incoming(program, state, source))
                    }
                    StoredOperand::Value { .. } | StoredOperand::Unavailable => false,
                }
            }
            if !complete_incoming(self.program, self.state, &incoming) {
                return;
            }
            if !self
                .context
                .control
                .operand_retentions
                .is_valid(capture.retention)
            {
                return;
            }
            let observations = self
                .context
                .control
                .statements
                .get(self.statement_fact)
                .value_observations;
            let mut reads = observations.handles().filter(|handle| {
                let read = self.context.control.value_observations.get(*handle);
                read.expression == capture.expression
                    && read.evaluation == self.evaluation
                    && read.receiver == capture.backing
                    && read.constraints == access.constraints
                    && !read.places.is_empty()
            });
            let Some(read) = reads.next() else {
                return;
            };
            if reads.next().is_some() {
                return;
            }
            capture.descriptor_permission =
                checked_trees::FlowDescriptorPermission::Loaded { loan: handle, read };
            capture.referents = self.append_referent_places(&[validation::CanonicalPlace {
                root: loan.root,
                segments: Vec::new(),
            }]);
            let formation_value = self.context.control.operands.append(*capture);
            self.context.borrow_lifetimes.activations.append(
                checked_trees::FlowBorrowActivationFact {
                    source: checked_trees::FlowInvalidationSource::Statement {
                        statement_index: self.statement_index,
                    },
                    loan: handle,
                    formation_value,
                },
            );
            crate::flow::reference_spans::append_constraint_ref(
                &mut self.context.contexts.constraint_refs,
                constraints,
                checked_trees::FlowConstraintKind::BorrowLoan { loan: handle },
            );
            capture.constraints = *constraints;
        }

        /*
            Observe the stored descriptors carried by one completed owned value.
            This is the same inventory used by selected payload copies; physical
            addresses are supplied by the evaluator, not recovered from an initializer.
        */
        pub(super) fn capture_owned_reference_leaves(
            &mut self,
            owner: Handle<FlowOperandFact>,
            places: &[CapturedReferencePlace],
        ) -> HandleSpan<ReferenceLeaf> {
            let capture = *self.context.control.operands.get(owner);
            let Some(paths) = borrow_carrying_owner_paths(self.program, capture.type_reference)
            else {
                return HandleSpan::empty();
            };
            let mut leaves = Vec::new();
            for path in paths {
                let path = crate::borrow::owner_place_segments(&path);
                let Some(reference) =
                    owned_projection_type_reference(self.program, capture.type_reference, &path)
                else {
                    return HandleSpan::empty();
                };
                if !self.is_view_type(reference) {
                    return HandleSpan::empty();
                }
                let suppliers = places
                    .iter()
                    .map(|place| {
                        let place = place.clone().projected(&path)?;
                        let stored = capture_at_snapshot(
                            self.program,
                            &self.context.control,
                            self.state,
                            self.storage_snapshot,
                            &place.place,
                        )?;
                        let captures = stored.captures()?;
                        (!captures.is_empty()).then_some(captures)
                    })
                    .collect::<Option<Vec<_>>>();
                let mut immutable_image = suppliers.as_ref().is_some_and(|rows| !rows.is_empty());
                if let Some(suppliers) = &suppliers {
                    for selected in suppliers.iter().flatten() {
                        let value = self.context.control.operands.get(selected.capture);
                        let profiles = captured_retention_profiles_at_path(
                            self.program,
                            &self.context.control,
                            value,
                            &selected.projection,
                        );
                        immutable_image &= profiles.is_some_and(|profiles| {
                            !profiles.is_empty()
                                && profiles.iter().all(|profile| {
                                    self.context.control.operand_retentions.is_valid(*profile)
                                        && self
                                            .context
                                            .control
                                            .operand_retentions
                                            .get(*profile)
                                            .immutable_image
                                        && self
                                            .context
                                            .control
                                            .retained_roots(*profile)
                                            .is_some_and(|roots| roots.is_empty())
                                })
                        });
                    }
                }
                let mut origins = Vec::new();
                let mut known = true;
                let mut lifetime_known = true;
                let mut saw_image = false;
                let mut retained_roots = Vec::new();
                for place in places {
                    let source = self
                        .append_captured_references(std::slice::from_ref(place))
                        .handles()
                        .next()
                        .unwrap_or_default();
                    let Some(place) = place.clone().projected(&path) else {
                        known = false;
                        break;
                    };
                    let lifetime = self
                        .stored_retention_at_place(&place.place)
                        .unwrap_or_else(|| self.captured_retention_at_path(capture, &path));
                    if let Some(roots) = self.context.control.retained_roots(lifetime) {
                        for root in roots {
                            if !retained_roots.contains(root) {
                                retained_roots.push(*root);
                            }
                        }
                    } else {
                        lifetime_known = false;
                    }
                    let Some(contents) = self.stored_reference_contents_at_place(&place.place)
                    else {
                        known = false;
                        break;
                    };
                    let alternatives = contents;
                    for contents in alternatives {
                        match contents {
                            StoredReferenceContents::Captured { referents, .. } => {
                                let resolved = self.captured_reference_places(referents);
                                if resolved.is_empty() {
                                    known = false;
                                    break;
                                }
                                origins.extend(resolved);
                            }
                            StoredReferenceContents::Incoming => {
                                let segments = self
                                    .context
                                    .control
                                    .operand_referent_segments
                                    .insert_many(place.place.segments.iter().copied());
                                let path = self
                                    .context
                                    .control
                                    .operand_child_paths
                                    .insert_many(path.iter().copied());
                                let formation = self.context.control.referent_formations.append(
                                    FlowReferentFormation {
                                        source,
                                        evidence: FormationEvidence::Access(capture.case_access),
                                        owner,
                                        path,
                                        root: place.place.root,
                                        segments,
                                    },
                                );
                                origins.push(CapturedReferencePlace {
                                    place: place.place.clone(),
                                    formation,
                                });
                            }
                            StoredReferenceContents::Image => saw_image = true,
                            StoredReferenceContents::Unknown => {
                                known = false;
                                break;
                            }
                        }
                    }
                    if !known {
                        break;
                    }
                }
                let referents = if known {
                    self.append_captured_references(&origins)
                } else {
                    HandleSpan::empty()
                };
                let retention = if lifetime_known && !places.is_empty() {
                    self.append_retention(&retained_roots)
                } else {
                    Handle::invalid()
                };
                if self.context.control.operand_retentions.is_valid(retention) {
                    let suppliers = suppliers
                        .map(|rows| {
                            let rows = rows
                                .into_iter()
                                .flatten()
                                .map(|selected| FlowOperandRetainedSupplier {
                                    contract: None,
                                    operand: selected.capture,
                                    projection: self
                                        .context
                                        .control
                                        .operand_retained_paths
                                        .insert_many(selected.projection),
                                })
                                .collect::<Vec<_>>();
                            self.context
                                .control
                                .operand_retained_suppliers
                                .insert_many(rows)
                        })
                        .unwrap_or_default();
                    let profile = self.context.control.operand_retentions.get_mut(retention);
                    profile.suppliers = suppliers;
                    profile.immutable_image =
                        immutable_image || (saw_image && origins.is_empty() && known);
                }
                let path = self.context.control.operand_child_paths.insert_many(path);
                leaves.push(ReferenceLeaf {
                    path,
                    type_reference: reference,
                    referents,
                    retention,
                    constraints: capture.constraints,
                });
            }
            self.context
                .control
                .captured_reference_leaves
                .insert_many(leaves)
        }

        /*
            Instantiate returned descriptor leaves at invocation entry. The body
            relation may name a descriptor saved before a later callee replacement;
            querying after those effects would substitute the wrong occurrence.
        */
        pub(super) fn capture_call_reference_leaves(
            &mut self,
            expression: ExpressionHandle,
            destination: TypeReferenceHandle,
            call: &TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            constraints: HandleSpan<FlowConstraintRef>,
        ) -> Option<HandleSpan<ReferenceLeaf>> {
            let returned = self.constructed_value_type(expression, destination)?;
            if self.is_view_type(returned) {
                return None;
            }
            let Some(paths) = borrow_carrying_owner_paths(self.program, returned) else {
                return Some(HandleSpan::empty());
            };
            if paths.is_empty() {
                return None;
            }
            let entered_target = validation::call_target_entry_symbol(
                self.program,
                validation::call_target_symbol(self.program, call),
            );
            let Some(parameters) = validation::find_state(self.program, entered_target)
                .map(|(_, state)| self.program.state_parameters(state))
                .or_else(|| {
                    validation::selected_requirement_signature(self.program, entered_target)
                        .map(|(_, signature)| self.program.state_signature_parameters(signature))
                })
            else {
                return Some(HandleSpan::empty());
            };
            let Some(indices) = self.captured_argument_indices(
                parameters,
                inputs
                    .iter()
                    .skip(usize::from(call.receiver.is_valid()))
                    .map(|(_, value)| value),
            ) else {
                return Some(HandleSpan::empty());
            };
            let Some(sources) = call_result_captured_sources(
                self.program,
                call,
                self.context.call_frames,
                &indices,
            ) else {
                return Some(HandleSpan::empty());
            };
            let mut leaves = Vec::new();
            for path in &paths {
                let path = crate::borrow::owner_place_segments(path);
                let Some(reference) =
                    owned_projection_type_reference(self.program, returned, &path)
                else {
                    return Some(HandleSpan::empty());
                };
                if !self.is_view_type(reference) {
                    return Some(HandleSpan::empty());
                }
                let mut places = Vec::new();
                let mut complete = true;
                let mut matched = false;
                for source in &sources {
                    let Some(output) =
                        loan_place_segments(self.program, returned, &source.result_path)
                    else {
                        return Some(HandleSpan::empty());
                    };
                    let suffix = match source.kind {
                        ReferenceResultSourceKind::Reference => {
                            if output != path {
                                continue;
                            }
                            Vec::new()
                        }
                        ReferenceResultSourceKind::Value { type_reference } => {
                            let Some(suffix) = path.strip_prefix(output.as_slice()) else {
                                continue;
                            };
                            let Some(suffix) = crate::borrow::structural_owner_path(
                                self.program,
                                &crate::borrow::owner_path_from_place_segments(
                                    self.program,
                                    suffix,
                                ),
                            ) else {
                                return Some(HandleSpan::empty());
                            };
                            let projected =
                                loan_place_segments(self.program, type_reference, &suffix)
                                    .and_then(|path| {
                                        owned_projection_type_reference(
                                            self.program,
                                            type_reference,
                                            &path,
                                        )
                                    });
                            if projected.is_none_or(|reference| !self.is_view_type(reference)) {
                                return Some(HandleSpan::empty());
                            }
                            suffix
                        }
                    };
                    matched = true;
                    match source.backing {
                        super::super::reference_places::ReferenceResultBacking::CallerArgument {
                            argument_index,
                            ..
                        } => {
                            let Some(actual) = inputs
                                .get(argument_index + usize::from(call.receiver.is_valid()))
                                .map(|(_, actual)| actual)
                            else {
                                complete = false;
                                break;
                            };
                            let selected = crate::borrow::structural_owner_path(
                                self.program,
                                &crate::borrow::owner_path_from_place_segments(
                                    self.program,
                                    &source.segments,
                                ),
                            )
                            .and_then(|mut path| {
                                path.extend_from_slice(&suffix);
                                loan_place_segments(self.program, actual.type_reference, &path)
                            });
                            let Some(selected) = selected else {
                                complete = false;
                                break;
                            };
                            let Some(projected) = self.capture_reference_projection(
                                actual,
                                &selected,
                                ReferenceProjection::Value,
                            ) else {
                                complete = false;
                                break;
                            };
                            if projected.places.is_empty() {
                                complete = false;
                                break;
                            }
                            for place in projected.places {
                                crate::shared::collections::push_unique(&mut places, place);
                            }
                        }
                        super::super::reference_places::ReferenceResultBacking::InvocationResult(
                            home,
                        ) => {
                            let selected = crate::borrow::structural_owner_path(
                                self.program,
                                &crate::borrow::owner_path_from_place_segments(
                                    self.program,
                                    &source.segments,
                                ),
                            )
                            .and_then(|mut path| {
                                path.extend_from_slice(&suffix);
                                loan_place_segments(self.program, home.referent, &path)
                            });
                            let Some(segments) = selected else {
                                complete = false;
                                break;
                            };
                            crate::shared::collections::push_unique(
                                &mut places,
                                CapturedReferencePlace::unformed(CanonicalPlace {
                                    root: PlaceRoot::Expression(expression),
                                    segments,
                                }),
                            );
                        }
                    }
                }
                let referents = if complete && matched {
                    self.append_captured_references(&places)
                } else {
                    HandleSpan::empty()
                };
                let path = self.context.control.operand_child_paths.insert_many(path);
                leaves.push(ReferenceLeaf {
                    path,
                    type_reference: reference,
                    referents,
                    retention: Handle::invalid(),
                    constraints,
                });
            }
            Some(
                self.context
                    .control
                    .captured_reference_leaves
                    .insert_many(leaves),
            )
        }

        /*
            Address capture precedes invocation effects; lifetime publication uses
            the result's complete declared profile, independently of that geometry.
        */
        pub(super) fn publish_returned_reference_leaves(
            &mut self,
            capture: &FlowOperandFact,
            references: HandleSpan<ReferenceLeaf>,
        ) -> HandleSpan<ReferenceLeaf> {
            let mut leaves = self
                .context
                .control
                .captured_reference_leaves
                .span_or_empty(references)
                .to_vec();
            for leaf in &mut leaves {
                let path = self
                    .context
                    .control
                    .operand_child_paths
                    .span_or_empty(leaf.path)
                    .to_vec();
                leaf.retention = self.captured_retention_at_path(*(capture), &path);
                leaf.constraints = capture.constraints;
            }
            self.context
                .control
                .captured_reference_leaves
                .insert_many(leaves)
        }

        pub(super) fn is_incoming_descriptor_place(&self, place: &CanonicalPlace) -> bool {
            if place.root.as_symbol().is_some_and(|symbol| {
                self.program
                    .state_parameter_by_symbol(self.state, symbol)
                    .is_some()
            }) {
                return true;
            }
            let Some((_, state)) = self
                .borrow
                .first_state_borrow(self.machine.symbol, self.state.symbol)
            else {
                return false;
            };
            self.borrow
                .loans
                .span_or_empty(state.loans)
                .iter()
                .any(|loan| {
                    if !self
                        .borrow
                        .incoming_references
                        .is_valid(loan.incoming_reference)
                    {
                        return false;
                    }
                    let incoming = self.borrow.incoming_references.get(loan.incoming_reference);
                    let root = incoming.place_root();
                    let Some(owner) = self.borrow.owner_segments.span(incoming.owner_path) else {
                        return false;
                    };
                    incoming.machine_symbol == self.machine.symbol
                        && incoming.state_symbol == self.state.symbol
                        && root == place.root
                        && owner.len() == place.segments.len()
                        && owner.iter().zip(&place.segments).all(|(owner, segment)| {
                            crate::borrow::owner_segment_matches_place_segment(
                                self.program,
                                owner,
                                segment,
                            ) == Some(true)
                        })
                })
        }

        /*
            Project an owned field from its receiver's completed address. Reading a
            view field instead loads its stored descriptor; the address of that
            descriptor is not the address it denotes.
        */
        pub(super) fn capture_member_referents(
            &mut self,
            expression: ExpressionHandle,
            receiver: &FlowOperandFact,
            constraints: HandleSpan<FlowConstraintRef>,
            case_access: &mut Handle<FlowCaseAccessFact>,
            possible_storage: &mut Option<HandleSpan<FlowOperandReferent>>,
            address_projection: &mut Option<FlowOperandAddressProjection>,
        ) -> HandleSpan<FlowOperandReferent> {
            let is_view = expression != self.reference_slot_target
                && self
                    .context
                    .expression_result_type_at(self.program, self.machine, self.state, expression)
                    .is_some_and(|reference| self.is_view_type(reference));
            let Some(member) = self.program.expression_table.member_node(expression) else {
                return HandleSpan::empty();
            };
            let symbol = effective_member_symbol(self.program, member.receiver, member);
            if !symbol.is_valid() {
                return HandleSpan::empty();
            }
            let mut path = Vec::new();
            if let Some(variant) = payload_variant_for_field(self.program, symbol) {
                path.push(PlaceSegment::Case { variant });
            }
            path.push(PlaceSegment::Field { symbol });
            *address_projection = Some(FlowOperandAddressProjection::Place(
                self.context
                    .control
                    .operand_child_paths
                    .insert_many(path.iter().copied()),
            ));
            let carrier = reference_shell(self.program, receiver.type_reference)
                .map_or(receiver.type_reference, |shell| shell.referee);
            if !is_view && owned_projection_type_reference(self.program, carrier, &path).is_some() {
                *possible_storage = self.project_possible_storage(receiver, &path);
            }

            let places = if !self.backing_allows_projection(receiver) {
                Some(Vec::new())
            } else {
                self.captured_reference_places(receiver.referents)
                    .into_iter()
                    .map(|place| place.projected(&path))
                    .collect::<Option<Vec<_>>>()
            };
            let Some(places) = places else {
                return HandleSpan::empty();
            };
            let source_places = self.append_captured_references(&places);
            let contexts = self
                .context
                .contexts
                .semantic_constraint_contexts(constraints)
                .collect::<Vec<_>>();
            let case_places = descriptor_observation_namespace(
                self.program,
                &self.context.control,
                receiver,
                &contexts,
            )
            .and_then(|namespace| project_captured_view_place(namespace, &path))
            .map(|namespace| self.append_referent_places(&[namespace]))
            .unwrap_or(source_places);
            *case_access = self.record_case_access_places(expression, constraints, case_places);
            if is_view {
                if let Some(captured) = self.captured_projection_referents(receiver, &path) {
                    return captured;
                }
                let value = self.context.control.operands.insert(*receiver);
                if let Some(selected) = reference_places_at_snapshot(
                    self.program,
                    &self.context.control,
                    value,
                    &path,
                    self.storage_snapshot,
                    |place| self.is_incoming_descriptor_place(place),
                ) {
                    return self.append_captured_references(&selected);
                }
                self.load_reference_places(places)
            } else {
                self.append_captured_references(&places)
            }
        }

        /*
            A completed owned value keeps its own descriptor leaves. A storage
            address instead loads the current descriptor after selector effects.
            Missing captured leaves are authoritative unknown, never a slot address.
        */
        fn captured_projection_referents(
            &mut self,
            receiver: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<HandleSpan<FlowOperandReferent>> {
            if self.is_view_type(receiver.type_reference)
                || (matches!(receiver.completion, FlowOperandCompletion::Observation)
                    && receiver.reference_leaves.is_empty())
            {
                return None;
            }
            let places = captured_operand_reference_places_with_formation(
                self.program,
                &self.context.control,
                receiver,
                receiver.type_reference,
                path,
            )
            .unwrap_or_default();
            Some(self.append_captured_references(&places))
        }

        fn load_reference_places(
            &mut self,
            sources: Vec<CapturedReferencePlace>,
        ) -> HandleSpan<FlowOperandReferent> {
            let mut places = Vec::new();
            for source in sources {
                let Some(contents) = self.stored_reference_contents_at_place(&source.place) else {
                    return HandleSpan::empty();
                };
                let alternatives = contents;
                for contents in alternatives {
                    match contents {
                        StoredReferenceContents::Captured { referents, .. } => {
                            let captured = self.captured_reference_places(referents);
                            if captured.is_empty() {
                                return HandleSpan::empty();
                            }
                            places.extend(captured);
                        }
                        StoredReferenceContents::Incoming => places.push(source.clone()),
                        StoredReferenceContents::Image => {}
                        StoredReferenceContents::Unknown => {
                            return HandleSpan::empty();
                        }
                    }
                }
            }
            self.append_captured_references(&places)
        }

        /*
            A checked return route substitutes the tuple that actually ran. Its
            reference arguments already retain their exact evaluated addresses,
            including anonymous owners and selectors evaluated before later inputs.
            Re-reading their source syntax would lose or replace those addresses.
            Lifetime retention and live loan authority remain independent checks.
        */
        pub(super) fn capture_returned_referents(
            &mut self,
            expression: ExpressionHandle,
            call: &TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            referents: &mut Option<HandleSpan<FlowOperandReferent>>,
        ) {
            let Some(reference) = self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                expression,
            ) else {
                return;
            };
            if !self.is_view_type(reference) {
                return;
            }
            *referents = Some(HandleSpan::empty());
            let entered_target = validation::call_target_entry_symbol(
                self.program,
                validation::call_target_symbol(self.program, call),
            );
            let Some(parameters) = validation::find_state(self.program, entered_target)
                .map(|(_, state)| self.program.state_parameters(state))
                .or_else(|| {
                    validation::selected_requirement_signature(self.program, entered_target)
                        .map(|(_, signature)| self.program.state_signature_parameters(signature))
                })
            else {
                return;
            };
            let Some(indices) = self.captured_argument_indices(
                parameters,
                inputs
                    .iter()
                    .skip(usize::from(call.receiver.is_valid()))
                    .map(|(_, value)| value),
            ) else {
                return;
            };
            let Some(sources) =
                call_result_leaf_sources(self.program, call, self.context.call_frames, &indices)
            else {
                return;
            };
            let mut origins = Vec::new();
            for source in sources {
                if !source.result_path.is_empty() {
                    return;
                }
                match source.backing {
                    super::super::reference_places::ReferenceResultBacking::CallerArgument {
                        argument_index,
                        ..
                    } => {
                        let Some((_, actual)) =
                            inputs.get(argument_index + usize::from(call.receiver.is_valid()))
                        else {
                            return;
                        };
                        let Some(projected) = self.capture_reference_projection(
                            actual,
                            &source.segments,
                            ReferenceProjection::Value,
                        ) else {
                            return;
                        };
                        let places = projected.places;
                        for place in places {
                            crate::shared::collections::push_unique(&mut origins, place);
                        }
                    }
                    super::super::reference_places::ReferenceResultBacking::InvocationResult(_) => {
                        crate::shared::collections::push_unique(
                            &mut origins,
                            CapturedReferencePlace::unformed(CanonicalPlace {
                                root: PlaceRoot::Expression(expression),
                                segments: source.segments,
                            }),
                        );
                    }
                }
            }
            *referents = Some(self.append_captured_references(&origins));
        }

        /*
            Normal return completes the descriptor formation established in the
            checked callee. The pre-call capture supplies possible backing, not a
            requirement that the caller select every mutually exclusive origin.
        */
        pub(super) fn complete_call_reference_formations(
            &mut self,
            call: Handle<FlowCallFact>,
            referents: &mut Option<HandleSpan<FlowOperandReferent>>,
            leaves: &mut Option<HandleSpan<ReferenceLeaf>>,
        ) {
            if !self.context.control.calls.is_valid(call) {
                return;
            }
            let mut returned_origins = Vec::new();
            if let Some(referents) = referents {
                *referents = self.returned_descriptor_formations(call, *referents);
                returned_origins.extend_from_slice(
                    self.context
                        .control
                        .operand_referents
                        .span_or_empty(*referents),
                );
            }
            if let Some(leaves) = leaves {
                let mut returned = self
                    .context
                    .control
                    .captured_reference_leaves
                    .span_or_empty(*leaves)
                    .to_vec();
                for leaf in &mut returned {
                    leaf.referents = self.returned_descriptor_formations(call, leaf.referents);
                    returned_origins.extend_from_slice(
                        self.context
                            .control
                            .operand_referents
                            .span_or_empty(leaf.referents),
                    );
                }
                *leaves = self
                    .context
                    .control
                    .captured_reference_leaves
                    .insert_many(returned);
            }
            let returned_origins = self
                .context
                .control
                .operand_referents
                .insert_many(returned_origins);
            self.context
                .control
                .calls
                .get_mut(call)
                .normal_return_referents = returned_origins;
        }

        fn returned_descriptor_formations(
            &mut self,
            call: Handle<FlowCallFact>,
            referents: HandleSpan<FlowOperandReferent>,
        ) -> HandleSpan<FlowOperandReferent> {
            let mut returned = Vec::new();
            for source in referents.handles() {
                let mut referent = *self.context.control.operand_referents.get(source);
                referent.formation =
                    self.context
                        .control
                        .referent_formations
                        .append(FlowReferentFormation {
                            source,
                            evidence: FormationEvidence::Return(call),
                            root: referent.root,
                            segments: referent.segments,
                            ..Default::default()
                        });
                returned.push(referent);
            }
            self.context.control.operand_referents.insert_many(returned)
        }

        /*
            Every covered arm must supply its completed descriptor. Keep formation
            identity with the address; equal paths can have different load obligations.
        */
        pub(super) fn capture_alternative_referents(
            &mut self,
            alternatives: HandleSpan<checked_trees::FlowOperandAlternative>,
        ) -> HandleSpan<FlowOperandReferent> {
            let Some(alternatives) = self.context.control.operand_alternatives.span(alternatives)
            else {
                return HandleSpan::empty();
            };
            if alternatives.is_empty() {
                return HandleSpan::empty();
            }
            let mut places = Vec::new();
            for alternative in alternatives {
                if !alternative.source_arm.is_valid()
                    || !self.context.control.operands.is_valid(alternative.value)
                {
                    return HandleSpan::empty();
                }
                let value = self.context.control.operands.get(alternative.value);
                let captured = self.captured_reference_places(value.referents);
                if captured.is_empty() {
                    return HandleSpan::empty();
                }
                places.extend(captured);
            }
            self.append_captured_references(&places)
        }

        /*
            Read the completed value installed by a reaching store. A matching
            unresolved capture or an intervening replacement is authoritative:
            neither may fall back to the declaration's initializer.
        */
        pub(super) fn stored_view_referents(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<HandleSpan<FlowOperandReferent>> {
            let source = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            )?;
            self.stored_view_referents_at_place(&source)
        }

        /*
            Resolve the actual stored descriptor through the current immutable cut.
            Expression and typed projection callers share this one association owner.
        */
        pub(super) fn stored_view_referents_at_place(
            &mut self,
            source: &CanonicalPlace,
        ) -> Option<HandleSpan<FlowOperandReferent>> {
            let contents = self.stored_reference_contents_at_place(source)?;
            let mut places = Vec::new();
            for alternative in contents {
                match alternative {
                    StoredReferenceContents::Incoming => {
                        places.push(CapturedReferencePlace::unformed(source.clone()))
                    }
                    StoredReferenceContents::Captured { referents, .. } => {
                        let captured = self.captured_reference_places(referents);
                        if captured.is_empty() {
                            return Some(HandleSpan::empty());
                        }
                        places.extend(captured);
                    }
                    StoredReferenceContents::Image => {}
                    StoredReferenceContents::Unknown => return Some(HandleSpan::empty()),
                }
            }
            Some(self.append_captured_references(&places))
        }

        /*
            Snapshot storage is the sole association owner. Unknown/malformed
            predecessors remain unknown; only an actual ingress keeps its namespace.
        */
        pub(super) fn stored_reference_contents_at_place(
            &mut self,
            source: &CanonicalPlace,
        ) -> Option<Vec<StoredReferenceContents>> {
            if source
                .segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
            {
                let root = CanonicalPlace {
                    root: source.root,
                    segments: Vec::new(),
                };
                let reference = canonical_place_type_reference(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    &root,
                )?;
                let paths = finite_projection_paths(self.program, reference, &source.segments)?;
                let mut alternatives = Vec::new();
                for segments in paths {
                    alternatives.extend(self.stored_reference_contents_at_place(
                        &CanonicalPlace {
                            root: source.root,
                            segments,
                        },
                    )?);
                }
                return (!alternatives.is_empty()).then_some(alternatives);
            }
            let Some(stored) = capture_at_snapshot(
                self.program,
                &self.context.control,
                self.state,
                self.storage_snapshot,
                source,
            ) else {
                return self
                    .is_incoming_descriptor_place(source)
                    .then_some(vec![StoredReferenceContents::Incoming]);
            };
            let mut pending = vec![stored];
            let mut alternatives = Vec::new();
            while let Some(stored) = pending.pop() {
                match stored {
                    StoredOperand::Unavailable => {
                        return Some(vec![StoredReferenceContents::Unknown]);
                    }
                    StoredOperand::Incoming(place) => {
                        if place != *source || !self.is_incoming_descriptor_place(&place) {
                            return Some(vec![StoredReferenceContents::Unknown]);
                        }
                        alternatives.push(StoredReferenceContents::Incoming);
                    }
                    StoredOperand::Alternatives(values) => {
                        pending.extend(values.into_iter().rev());
                    }
                    StoredOperand::Value {
                        capture,
                        projection,
                        ..
                    } => {
                        let places = reference_places_at_snapshot(
                            self.program,
                            &self.context.control,
                            capture,
                            &projection,
                            self.storage_snapshot,
                            |place| self.is_incoming_descriptor_place(place),
                        )
                        .unwrap_or_default();
                        if places.is_empty()
                            && projection_is_immutable_image(
                                self.program,
                                &self.context.control,
                                capture,
                                &projection,
                            )
                        {
                            alternatives.push(StoredReferenceContents::Image);
                        } else {
                            let referents = self.append_captured_references(&places);
                            alternatives.push(StoredReferenceContents::Captured { referents });
                        }
                    }
                }
            }
            (!alternatives.is_empty()).then_some(alternatives)
        }

        pub(super) fn capture_indexed_referents(
            &mut self,
            expression: ExpressionHandle,
            collection: &FlowOperandFact,
            start: &FlowOperandFact,
            end: Option<&FlowOperandFact>,
            possible_storage: &mut Option<HandleSpan<FlowOperandReferent>>,
            address_projection: &mut Option<FlowOperandAddressProjection>,
        ) -> HandleSpan<FlowOperandReferent> {
            if !indexed_operation_has_builtin_meaning(
                self.program,
                self.machine,
                Some(self.state),
                expression,
                Some(collection.type_reference),
            ) || [Some(start), end].into_iter().flatten().any(|bound| {
                bound.expression.is_valid()
                    && !has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        bound.expression,
                    )
            }) {
                return HandleSpan::empty();
            }
            for operand in [Some(start), end].into_iter().flatten() {
                if operand.expression.is_valid() {
                    self.context.control.operands.insert(*operand);
                }
            }
            let indexed = self.program.expression_table.indexed_node(expression);
            let Some(indexed) = indexed else {
                return HandleSpan::empty();
            };
            let mut places = self.captured_collection_places(collection);
            let geometry = if let Some(end) = end {
                let Some(range) = self.program.expression_table.range_node(indexed.index) else {
                    return HandleSpan::empty();
                };
                let start_coordinate = if start.expression.is_valid() {
                    self.captured_coordinate(start)
                } else {
                    Some(0)
                };
                if start_coordinate == Some(0) && !end.expression.is_valid() && !range.end_inclusive
                {
                    *address_projection =
                        Some(FlowOperandAddressProjection::View(HandleSpan::empty()));
                    *possible_storage = self.project_possible_storage(collection, &[]);
                    return self.append_captured_references(&places);
                }
                let end_coordinate = if end.expression.is_valid() {
                    self.captured_coordinate(end).and_then(|end| {
                        if range.end_inclusive {
                            end.checked_add(1)
                        } else {
                            Some(end)
                        }
                    })
                } else {
                    let mut extent = None;
                    for place in &places {
                        let Some(current) = crate::checks::collection_extents::at_formed_place(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            &place.place,
                        ) else {
                            return HandleSpan::empty();
                        };
                        if extent.is_some_and(|previous| previous != current) {
                            return HandleSpan::empty();
                        }
                        extent = Some(current);
                    }
                    extent
                };
                match (start_coordinate, end_coordinate) {
                    (Some(start), Some(end)) if start <= end => {
                        PlaceSegment::FixedRange { start, end }
                    }
                    _ => PlaceSegment::OpaqueRange,
                }
            } else {
                match self.captured_coordinate(start) {
                    Some(index) => PlaceSegment::FixedIndex { index },
                    None => PlaceSegment::Index {
                        expression: indexed.index,
                    },
                }
            };
            let segments = self
                .context
                .control
                .operand_child_paths
                .insert_many([geometry]);
            *address_projection = Some(if end.is_some() {
                FlowOperandAddressProjection::View(segments)
            } else {
                FlowOperandAddressProjection::Place(segments)
            });
            let reads_descriptor = end.is_none()
                && expression != self.reference_slot_target
                && self
                    .context
                    .expression_result_type_at(self.program, self.machine, self.state, expression)
                    .is_some_and(|reference| self.is_view_type(reference));
            if !reads_descriptor {
                *possible_storage = self.project_possible_storage(collection, &[geometry]);
            }
            if reads_descriptor
                && let Some(referents) = self.captured_projection_referents(collection, &[geometry])
            {
                return referents;
            }
            for place in &mut places {
                let Some(segments) =
                    project_captured_view_segments(&place.place.segments, &[geometry])
                else {
                    return HandleSpan::empty();
                };
                place.place.segments = segments;
            }
            if reads_descriptor {
                self.load_reference_places(places)
            } else {
                self.append_captured_references(&places)
            }
        }

        /*
            Both descriptor stores and returned values consume this exact completed
            argument roster. Formal carrier checks decide eligible integer lanes;
            captured_coordinate reads their immutable observations, not current homes.
        */
        pub(super) fn captured_argument_indices<'inputs>(
            &self,
            parameters: &[StateParameter],
            inputs: impl ExactSizeIterator<Item = &'inputs FlowOperandFact>,
        ) -> Option<Vec<Option<usize>>> {
            if parameters.len() != inputs.len() {
                return None;
            }
            Some(
                parameters
                    .iter()
                    .zip(inputs)
                    .map(|(parameter, input)| {
                        if reference_shell(self.program, parameter.type_reference).is_none()
                            && self
                                .program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some_and(|primitive| primitive.is_fixed_integer())
                        {
                            self.captured_coordinate(input)
                        } else {
                            None
                        }
                    })
                    .collect(),
            )
        }

        pub(super) fn captured_coordinate(&self, operand: &FlowOperandFact) -> Option<usize> {
            captured_integer_value(
                self.program,
                self.machine,
                self.state,
                operand.expression,
                |expression, place| {
                    if !matches!(place.root, PlaceRoot::Symbol(_))
                        && place
                            .root
                            .as_expression()
                            .and_then(|expression| {
                                declared_place_type_raw(
                                    self.program,
                                    self.machine,
                                    Some(self.state),
                                    expression,
                                )
                            })
                            .is_none()
                    {
                        return None;
                    }
                    let place = completed_copy_read_place(
                        self.program,
                        &self.context.control,
                        self.context.active_completed_copies,
                        self.state.symbol,
                        self.statement_index,
                        expression,
                        place,
                    )?;
                    let value = crate::values::scalar_value_at_place(
                        self.program,
                        self.semantic,
                        self.context
                            .contexts
                            .constraint_refs
                            .span_or_empty(operand.constraints)
                            .iter()
                            .filter_map(|reference| {
                                let FlowConstraintKind::SemanticContext { context } =
                                    reference.kind
                                else {
                                    return None;
                                };
                                Some(self.semantic.contexts.get(context))
                            }),
                        &place,
                    )?;
                    match value {
                        ScalarValue::Integer(value) => Some(value),
                        _ => None,
                    }
                },
            )?
            .to_u64()
            .and_then(|value| usize::try_from(value).ok())
        }

        fn captured_collection_places(
            &self,
            operand: &FlowOperandFact,
        ) -> Vec<CapturedReferencePlace> {
            if !self.backing_allows_projection(operand) {
                Vec::new()
            } else {
                self.captured_reference_places(operand.referents)
            }
        }

        pub(super) fn captured_reference_places(
            &self,
            referents: HandleSpan<FlowOperandReferent>,
        ) -> Vec<CapturedReferencePlace> {
            self.context
                .control
                .operand_referents
                .span_or_empty(referents)
                .iter()
                .map(|referent| {
                    CapturedReferencePlace::from_referent(&self.context.control, referent)
                })
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default()
        }

        pub(super) fn append_captured_references(
            &mut self,
            places: &[CapturedReferencePlace],
        ) -> HandleSpan<FlowOperandReferent> {
            let mut rows = Vec::new();
            let mut seen = Vec::new();
            for reference in places {
                if seen.contains(&reference) {
                    continue;
                }
                seen.push(reference);
                let segments = self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(reference.place.segments.iter().copied());
                rows.push(FlowOperandReferent {
                    root: reference.place.root,
                    segments,
                    formation: reference.formation,
                });
            }
            self.context.control.operand_referents.insert_many(rows)
        }

        /*
            A real descriptor load completes its pending owner-case obligation at
            this read. Its endpoint becomes immutable provenance; later pointee
            projections still consume current contents and current case evidence.
        */
        pub(super) fn complete_reference_formation(
            &mut self,
            expression: ExpressionHandle,
            constraints: HandleSpan<FlowConstraintRef>,
            referents: HandleSpan<FlowOperandReferent>,
            source_access: arena::Handle<checked_trees::FlowCaseAccessFact>,
        ) -> HandleSpan<FlowOperandReferent> {
            if referents.is_empty() && self.context.control.case_accesses.is_valid(source_access) {
                let access = self.context.control.case_accesses.get(source_access);
                if access.machine_symbol == self.machine.symbol
                    && access.state_symbol == self.state.symbol
                    && access.statement_index == self.statement_index
                    && access.expression == expression
                    && access.evaluation == self.evaluation
                    && access.constraints == constraints
                    && !access.places.is_empty()
                {
                    return referents;
                }
            }
            let access = self.record_case_access_places(expression, constraints, referents);
            if !access.is_valid() {
                return referents;
            }
            let mut completed = Vec::new();
            for source in referents.handles() {
                let Some(mut reference) = CapturedReferencePlace::from_referent(
                    &self.context.control,
                    self.context.control.operand_referents.get(source),
                ) else {
                    return HandleSpan::empty();
                };
                let segments = self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(reference.place.segments.iter().copied());
                reference.formation =
                    self.context
                        .control
                        .referent_formations
                        .append(FlowReferentFormation {
                            source,
                            evidence: FormationEvidence::Access(access),
                            owner: Handle::invalid(),
                            path: HandleSpan::empty(),
                            root: reference.place.root,
                            segments,
                        });
                completed.push(reference);
            }
            self.append_captured_references(&completed)
        }

        pub(super) fn captured_referent_places(
            &self,
            referents: HandleSpan<FlowOperandReferent>,
        ) -> Vec<CanonicalPlace> {
            self.context
                .control
                .operand_referents
                .span_or_empty(referents)
                .iter()
                .map(|referent| CanonicalPlace {
                    root: referent.root,
                    segments: self
                        .context
                        .control
                        .operand_referent_segments
                        .span_or_empty(referent.segments)
                        .to_vec(),
                })
                .collect()
        }

        pub(super) fn append_referent_places(
            &mut self,
            places: &[CanonicalPlace],
        ) -> HandleSpan<FlowOperandReferent> {
            let rows = places
                .iter()
                .map(|place| FlowOperandReferent {
                    formation: Handle::invalid(),
                    root: place.root,
                    segments: self
                        .context
                        .control
                        .operand_referent_segments
                        .insert_many(place.segments.iter().copied()),
                })
                .collect::<Vec<_>>();
            self.context.control.operand_referents.insert_many(rows)
        }
    }

    /*
        An immutable image projected through a completed constructor has no local
        address. Empty referents are that image, not a missing supplier.
    */
    fn projection_is_immutable_image(
        program: &dyn validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        capture: Handle<checked_trees::FlowOperandFact>,
        projection: &[PlaceSegment],
    ) -> bool {
        if !control.operands.is_valid(capture) {
            return false;
        }
        let operand = control.operands.get(capture);
        if projection.is_empty() {
            return operand.referents.is_empty()
                && control.operand_retentions.is_valid(operand.retention)
                && control
                    .operand_retentions
                    .get(operand.retention)
                    .immutable_image;
        }
        let Some((child, _, consumed)) =
            validation::completed_storage::captured_operand_child_at_path(
                program,
                control,
                operand,
                operand.type_reference,
                projection,
            )
        else {
            return false;
        };
        projection_is_immutable_image(program, control, child, &projection[consumed..])
    }
}
mod view_retention {
    /*
        Captured lifetime dependencies are independent of view address geometry.
        Declaration relations keep every input lifetime, including inputs a body
        does not select. Actual operand captures substitute those lifetimes once;
        they never turn a declaration fallback into an address or extent proof.
        An unchanged state input already owns its admitted reference frontier.
        Retain those leaves against the input owner even when no local store has
        produced a capture; selecting a case payload cannot erase that dependency.
    */
    use super::super::canonical_place_segments_equal;
    use super::super::canonical_place_segments_may_overlap;
    use super::super::operand_values::captured_lifetime_roots;
    use super::super::operand_values::captured_retention_profiles_at_path;
    use super::super::stored_values::StoredObservation;
    use super::super::stored_values::StoredOperand;
    use super::super::stored_values::at_snapshot;
    use super::super::stored_views::capture_at_snapshot;
    use super::Execution;
    use crate::published::places::place_at;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        BorrowLoanOwnerSegment, FlowOperandAlternative, FlowOperandCompletion, FlowOperandFact,
        FlowOperandLifetimeContract, FlowOperandReferent, FlowOperandRetainedLeaf,
        FlowOperandRetainedSupplier, FlowOperandRetention, FlowStorageSnapshot,
    };
    use facts::{PlaceRoot, PlaceSegment, payload_variant_for_field};
    use typed_trees::expression::{ExpressionHandle, TableCallExpression};
    use typed_trees::types::TypeReferenceHandle;
    use validation::borrow_contracts::ViewReturnInputSource;
    use validation::borrow_contracts::borrow_carrying_owner_paths;
    use validation::reference_result_custody::input_carrier_type;
    use validation::reference_result_custody::loan_place_segments;
    use validation::{
        CanonicalPlace, call_target_entry_symbol, call_target_return_type, call_target_symbol,
        effective_member_symbol, value_element_has_builtin_coordinate,
    };

    impl Execution<'_, '_, '_> {
        /*
            A reference value retains both its actual storage lifetime and the
            admitted contents of an incoming carrier. Completed values and reaching
            stores remain authoritative, including explicit unknown profiles.
            Incoming contents supply lifetime debt only: no child evaluation,
            descriptor address or permission is manufactured from their type.
        */
        pub(super) fn captured_reference_retention(
            &mut self,
            capture: &FlowOperandFact,
            completed: Option<Handle<FlowOperandRetention>>,
        ) -> Option<Handle<FlowOperandRetention>> {
            completed
                .or_else(|| self.stored_operand_retention(capture.expression))
                .or_else(|| {
                    let source = place_at(
                        self.program,
                        self.state.symbol,
                        capture.statement_index,
                        capture.expression,
                    )?;
                    let carrier = input_carrier_type(self.program, capture.type_reference)?;
                    if borrow_carrying_owner_paths(self.program, carrier)?.is_empty() {
                        return None;
                    }
                    let retention =
                        self.incoming_value_retention(&source, capture.storage_snapshot)?;
                    let path = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(source.segments.iter().copied());
                    let contents = FlowOperandFact {
                        type_reference: carrier,
                        state_symbol: capture.state_symbol,
                        statement_index: capture.statement_index,
                        storage_snapshot: capture.storage_snapshot,
                        constraints: capture.constraints,
                        entry_origin_writes: capture.entry_origin_writes,
                        entry_origin_unknown_write: capture.entry_origin_unknown_write,
                        entry_binding_writes: capture.entry_binding_writes,
                        entry_binding_unknown_write: capture.entry_binding_unknown_write,
                        retention,
                        completion: FlowOperandCompletion::IncomingContents {
                            root: source.root,
                            path,
                        },
                        ..Default::default()
                    };
                    let backing = self.context.control.operands.insert(*capture);
                    Some(self.capture_storage_retention(&contents, Some(retention), backing))
                })
        }

        /*
            Forming a view retains its evaluated address, independently of whether
            the contents of that storage have a known completed-value profile.
            Preserve that profile for later projections, but never use an unknown
            descriptor value to erase the backing of an actually formed reborrow.
        */
        pub(super) fn formed_view_retention(
            &mut self,
            backing: &FlowOperandFact,
        ) -> Handle<FlowOperandRetention> {
            if backing.referents.is_empty() {
                return backing.retention;
            }
            let lifetime = self.retention_from_referents(backing.referents);
            let roots = self.context.control.operand_retentions.get(lifetime).roots;
            let stored_value = self.context.control.operands.insert(*backing);
            let enclosing_storage = self.view_enclosure(backing.retention);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    roots,
                    stored_value,
                    enclosing_storage,
                    ..Default::default()
                })
        }

        fn retention_from_stored_operand(
            &mut self,
            stored: StoredOperand,
        ) -> Handle<FlowOperandRetention> {
            let mut pending = vec![stored];
            let mut profiles = Vec::new();
            while let Some(stored) = pending.pop() {
                let profile = match stored {
                    StoredOperand::Unavailable => {
                        return Handle::invalid();
                    }
                    StoredOperand::Incoming(place) => {
                        if !self.is_incoming_descriptor_place(&place)
                            || !place.root.as_symbol().is_some_and(|symbol| {
                                self.program
                                    .state_parameter_by_symbol(self.state, symbol)
                                    .is_some()
                            })
                        {
                            return Handle::invalid();
                        }
                        self.append_retention(&[place.root])
                    }
                    StoredOperand::Alternatives(values) => {
                        pending.extend(values.into_iter().rev());
                        continue;
                    }
                    StoredOperand::Value {
                        capture,
                        projection,
                        ..
                    } => self.stored_capture_retention(capture, &projection),
                };
                profiles.push(profile);
            }
            self.join_retention_profiles(&profiles)
        }

        /*
            Reading a saved reference keeps the completed value that supplied it.
            Its lifetime roots alone cannot identify the permission after the old
            variable changes. A fresh profile points to the existing capture;
            neither the original profile nor the evaluation occurrence is changed.
        */
        fn stored_capture_retention(
            &mut self,
            capture: Handle<FlowOperandFact>,
            projection: &[PlaceSegment],
        ) -> Handle<FlowOperandRetention> {
            let value = *self.context.control.operands.get(capture);
            let retained = self.captured_retention_at_path(value, projection);
            if !self.context.control.operand_retentions.is_valid(retained)
                || !validation::project_type_reference_from_segments(
                    self.program,
                    value.type_reference,
                    projection,
                )
                .is_some_and(|reference| {
                    validation::reference_shell(self.program, reference).is_some()
                })
            {
                return retained;
            }
            let projection = self
                .context
                .control
                .operand_retained_paths
                .insert_many(projection.iter().copied());
            let suppliers = self
                .context
                .control
                .operand_retained_suppliers
                .insert_many([FlowOperandRetainedSupplier {
                    operand: capture,
                    projection,
                    contract: None,
                }]);
            let profile = *self.context.control.operand_retentions.get(retained);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    suppliers,
                    ..profile
                })
        }

        /*
            Keep every completed descriptor profile at a selection. The joined
            roots answer lifetime-only queries; alternatives retain the actual
            stored contents needed by a later projection through a descriptor.
        */
        pub(super) fn captured_retention_at_path(
            &mut self,
            capture: FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Handle<FlowOperandRetention> {
            let Some(profiles) = captured_retention_profiles_at_path(
                self.program,
                &self.context.control,
                &capture,
                path,
            ) else {
                return Handle::invalid();
            };
            self.join_retention_profiles(&profiles)
        }

        pub(super) fn join_retention_profiles(
            &mut self,
            profiles: &[Handle<FlowOperandRetention>],
        ) -> Handle<FlowOperandRetention> {
            if let [profile] = profiles {
                return *profile;
            }
            if profiles.is_empty() {
                return Handle::invalid();
            }
            let mut roots = Vec::new();
            let mut alternatives = Vec::new();
            let mut immutable_image = true;
            for profile in profiles {
                immutable_image &= self.context.control.operand_retentions.is_valid(*profile)
                    && self
                        .context
                        .control
                        .operand_retentions
                        .get(*profile)
                        .immutable_image;
                let Some(retained) = self.context.control.retained_roots(*profile) else {
                    return Handle::invalid();
                };
                for root in retained {
                    if !roots.contains(root) {
                        roots.push(*root);
                    }
                }
                alternatives.push(*self.context.control.operand_retentions.get(*profile));
            }
            let roots = self
                .context
                .control
                .operand_retained_roots
                .insert_many(roots);
            let alternatives = self
                .context
                .control
                .operand_retentions
                .insert_many(alternatives);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    immutable_image,
                    roots,
                    alternatives,
                    ..Default::default()
                })
        }

        /*
            Every possible result must cover each declared lifetime leaf at its
            own completion. Joining dependencies never joins address geometry or
            turns mutually exclusive values into simultaneous product children.
        */
        pub(super) fn capture_alternative_retention(
            &mut self,
            expression: ExpressionHandle,
            alternatives: HandleSpan<FlowOperandAlternative>,
        ) -> Handle<FlowOperandRetention> {
            let Some(reference) = self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                expression,
            ) else {
                return Handle::invalid();
            };
            let Some(rows) = self.context.control.operand_alternatives.span(alternatives) else {
                return Handle::invalid();
            };
            if rows.is_empty()
                || rows.iter().any(|row| {
                    !row.source_arm.is_valid() || !self.context.control.operands.is_valid(row.value)
                })
            {
                return Handle::invalid();
            }
            let alternatives = rows
                .iter()
                .map(|row| (*self.context.control.operands.get(row.value), Vec::new()))
                .collect::<Vec<_>>();
            self.capture_retention_join(reference, &alternatives)
        }

        /*
            Join complete declared lifetime leaves across selected completed
            values. Branch results and runtime element choices differ in geometry,
            not in the requirement that every candidate cover each retained leaf.
        */
        fn capture_retention_join(
            &mut self,
            reference: TypeReferenceHandle,
            selections: &[(FlowOperandFact, Vec<PlaceSegment>)],
        ) -> Handle<FlowOperandRetention> {
            let Some(paths) = borrow_carrying_owner_paths(self.program, reference) else {
                return Handle::invalid();
            };
            if selections.is_empty() {
                return Handle::invalid();
            }
            let mut leaves = Vec::new();
            for path in paths {
                let path = crate::borrow::owner_place_segments(&path);
                let mut profiles = Vec::new();
                for (capture, prefix) in selections {
                    let mut selected_path = prefix.clone();
                    selected_path.extend_from_slice(&path);
                    let leaf = self.captured_retention_at_path(*(capture), &selected_path);
                    if self.context.control.retained_roots(leaf).is_none() {
                        return Handle::invalid();
                    }
                    crate::shared::collections::push_unique(&mut profiles, leaf);
                }
                let retention = self.join_retention_profiles(&profiles);
                if path.is_empty() {
                    return retention;
                }
                let path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(path);
                leaves.push(FlowOperandRetainedLeaf { path, retention });
            }
            let leaves = self
                .context
                .control
                .operand_retained_leaves
                .insert_many(leaves);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    roots: HandleSpan::empty(),
                    alternatives: HandleSpan::empty(),
                    leaves,
                    stored_value: Handle::invalid(),
                    stored_projection: HandleSpan::empty(),
                    ..Default::default()
                })
        }

        pub(super) fn append_retention(
            &mut self,
            roots: &[PlaceRoot],
        ) -> Handle<FlowOperandRetention> {
            self.insert_retention(roots.iter().copied(), Handle::invalid())
        }

        /*
            One retention over exact `roots` and one stored value, with no
            alternative or leaf roster: the profile covers that value alone.
        */
        fn insert_retention(
            &mut self,
            roots: impl IntoIterator<Item = PlaceRoot>,
            stored_value: Handle<FlowOperandFact>,
        ) -> Handle<FlowOperandRetention> {
            let roots = self
                .context
                .control
                .operand_retained_roots
                .insert_many(roots);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    roots,
                    alternatives: HandleSpan::empty(),
                    leaves: HandleSpan::empty(),
                    stored_value,
                    stored_projection: HandleSpan::empty(),
                    ..Default::default()
                })
        }

        pub(super) fn retention_from_referents(
            &mut self,
            referents: HandleSpan<FlowOperandReferent>,
        ) -> Handle<FlowOperandRetention> {
            let mut roots = Vec::new();
            for referent in self
                .context
                .control
                .operand_referents
                .span_or_empty(referents)
            {
                crate::shared::collections::push_unique(&mut roots, referent.root);
            }
            if roots.is_empty() {
                Handle::invalid()
            } else {
                self.append_retention(&roots)
            }
        }

        /*
            Keep the authenticated stored value and its selected region together.
            No child capture or geometric observation is manufactured by this link.
        */
        fn projected_value_retention(
            &mut self,
            capture: FlowOperandFact,
            projection: &[PlaceSegment],
        ) -> Handle<FlowOperandRetention> {
            let stored_value = self.context.control.operands.insert(capture);
            let stored_projection = self
                .context
                .control
                .operand_retained_paths
                .insert_many(projection.iter().copied());
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    roots: HandleSpan::empty(),
                    alternatives: HandleSpan::empty(),
                    leaves: HandleSpan::empty(),
                    stored_value,
                    stored_projection,
                    ..Default::default()
                })
        }

        pub(super) fn stored_retention_at_place(
            &mut self,
            source: &CanonicalPlace,
        ) -> Option<Handle<FlowOperandRetention>> {
            let capture = capture_at_snapshot(
                self.program,
                &self.context.control,
                self.state,
                self.storage_snapshot,
                source,
            )?;
            Some(match capture {
                stored @ (StoredOperand::Unavailable
                | StoredOperand::Incoming(_)
                | StoredOperand::Alternatives(_)) => self.retention_from_stored_operand(stored),
                StoredOperand::Value {
                    capture,
                    projection,
                    ..
                } => self.stored_capture_retention(capture, &projection),
            })
        }

        pub(super) fn stored_operand_retention(
            &mut self,
            expression: ExpressionHandle,
        ) -> Option<Handle<FlowOperandRetention>> {
            let source = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            )?;
            self.stored_retention_at_place(&source)
        }

        /*
            State admission establishes each reference leaf of its exact input.
            A whole-owner binding query ignores descendant field stores, so check
            each leaf's reaching value independently: replacements retain their
            completed capture, unknown writes stay unknown, and only an unchanged
            leaf retains the input owner. None supplies an address or new access.
        */
        fn incoming_value_retention(
            &mut self,
            source: &CanonicalPlace,
            snapshot: Handle<FlowStorageSnapshot>,
        ) -> Option<Handle<FlowOperandRetention>> {
            let parameter = self
                .program
                .state_parameter_by_symbol(self.state, source.root.as_symbol()?)?;
            let carrier = input_carrier_type(self.program, parameter.type_reference)?;
            let paths = borrow_carrying_owner_paths(self.program, carrier)?;
            let mut leaves = Vec::new();
            for path in paths {
                let path = crate::borrow::owner_place_segments(&path);
                if path.len() < source.segments.len()
                    || !source.segments.iter().zip(&path).all(|(left, right)| {
                        canonical_place_segments_equal(self.program, *left, *right)
                    })
                {
                    continue;
                }
                let leaf = CanonicalPlace {
                    root: source.root,
                    segments: path.clone(),
                };
                let retention = match capture_at_snapshot(
                    self.program,
                    &self.context.control,
                    self.state,
                    snapshot,
                    &leaf,
                ) {
                    Some(StoredOperand::Value {
                        capture,
                        projection,
                        ..
                    }) => self.stored_capture_retention(capture, &projection),
                    Some(
                        stored @ (StoredOperand::Unavailable
                        | StoredOperand::Incoming(_)
                        | StoredOperand::Alternatives(_)),
                    ) => self.retention_from_stored_operand(stored),
                    None => self.append_retention(&[source.root]),
                };
                let remaining = &path[source.segments.len()..];
                if remaining.is_empty() {
                    return Some(retention);
                }
                let path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(remaining.iter().copied());
                leaves.push(FlowOperandRetainedLeaf { path, retention });
            }
            if leaves.is_empty() {
                return None;
            }
            let leaves = self
                .context
                .control
                .operand_retained_leaves
                .insert_many(leaves);
            Some(
                self.context
                    .control
                    .operand_retentions
                    .insert(FlowOperandRetention {
                        roots: HandleSpan::empty(),
                        alternatives: HandleSpan::empty(),
                        leaves,
                        stored_value: Handle::invalid(),
                        stored_projection: HandleSpan::empty(),
                        ..Default::default()
                    }),
            )
        }

        pub(super) fn capture_storage_retention(
            &mut self,
            capture: &FlowOperandFact,
            value_retention: Option<Handle<FlowOperandRetention>>,
            backing: Handle<FlowOperandFact>,
        ) -> Handle<FlowOperandRetention> {
            let backing_fact = if backing.is_valid() {
                if !self.context.control.operands.is_valid(backing) {
                    return Handle::invalid();
                }
                Some(*self.context.control.operands.get(backing))
            } else {
                None
            };
            let storage_retention = backing_fact
                .map(|fact| fact.retention)
                .unwrap_or(capture.retention);
            let reference_free = borrow_carrying_owner_paths(self.program, capture.type_reference)
                .is_some_and(|paths| paths.is_empty());
            let roots = if let Some(roots) = self
                .context
                .control
                .retained_roots(storage_retention)
                .map(<[_]>::to_vec)
            {
                roots
            } else if reference_free
                && let Some(fact) = backing_fact
                && self.is_view_type(fact.type_reference)
            {
                let mut roots = captured_lifetime_roots(self.program, &self.context.control, &fact)
                    .filter(|roots| !roots.is_empty())
                    .unwrap_or_default();
                if roots.is_empty() {
                    for place in self.captured_referent_places(fact.referents) {
                        if place.root.is_valid() && !roots.contains(&place.root) {
                            roots.push(place.root);
                        }
                    }
                }
                if roots.is_empty() {
                    return Handle::invalid();
                }
                roots
            } else {
                return Handle::invalid();
            };
            if reference_free && matches!(capture.completion, FlowOperandCompletion::Observation) {
                let lifetime = self.append_retention(&roots);
                let enclosing_storage = self.view_enclosure(storage_retention);
                self.context
                    .control
                    .operand_retentions
                    .get_mut(lifetime)
                    .enclosing_storage = enclosing_storage;
                return lifetime;
            }
            let value_retention = value_retention.or_else(|| {
                (!self.is_view_type(capture.type_reference))
                    .then(|| self.capture_value_retention(capture))
            });
            let mut value = *capture;
            if let Some(retention) = value_retention {
                value.retention = retention;
            }
            if matches!(capture.completion, FlowOperandCompletion::Observation)
                && value_retention.is_none()
                && let Some(source) = place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    capture.expression,
                )
            {
                match capture_at_snapshot(
                    self.program,
                    &self.context.control,
                    self.state,
                    self.storage_snapshot,
                    &source,
                ) {
                    Some(StoredOperand::Value {
                        capture,
                        projection,
                        ..
                    }) => {
                        let capture = *self.context.control.operands.get(capture);
                        if projection.is_empty() {
                            value = capture;
                        } else {
                            value.retention = self.projected_value_retention(capture, &projection);
                        }
                    }
                    Some(_) => return self.append_retention(&roots),
                    None => {
                        if let Some(retention) =
                            self.incoming_value_retention(&source, capture.storage_snapshot)
                        {
                            value.retention = retention;
                        }
                    }
                }
            }
            let stored_value = self.context.control.operands.insert(value);
            self.insert_retention(roots, stored_value)
        }

        pub(super) fn capture_value_retention(
            &mut self,
            capture: &FlowOperandFact,
        ) -> Handle<FlowOperandRetention> {
            let Some(paths) = borrow_carrying_owner_paths(self.program, capture.type_reference)
            else {
                return Handle::invalid();
            };
            if paths.is_empty() {
                return Handle::invalid();
            }
            let mut value = *capture;
            if capture.constructed_children().is_empty() {
                let Some(source) = place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    capture.expression,
                ) else {
                    return Handle::invalid();
                };
                match at_snapshot(
                    self.program,
                    &self.context.control,
                    capture.storage_snapshot,
                    &source,
                    StoredObservation::Contents,
                ) {
                    Some(StoredOperand::Value {
                        capture,
                        projection,
                        ..
                    }) => {
                        let capture = *self.context.control.operands.get(capture);
                        if projection.is_empty() {
                            value = capture;
                        } else {
                            value.retention = self.projected_value_retention(capture, &projection);
                        }
                    }
                    None => {
                        let Some(retention) =
                            self.incoming_value_retention(&source, capture.storage_snapshot)
                        else {
                            return Handle::invalid();
                        };
                        value.retention = retention;
                    }
                    Some(stored @ StoredOperand::Alternatives(_)) => {
                        let Some(captures) = stored.captures() else {
                            return Handle::invalid();
                        };
                        let selections = captures
                            .into_iter()
                            .map(|selected| {
                                (
                                    *self.context.control.operands.get(selected.capture),
                                    selected.projection,
                                )
                            })
                            .collect::<Vec<_>>();
                        value.retention =
                            self.capture_retention_join(value.type_reference, &selections);
                    }
                    Some(StoredOperand::Unavailable | StoredOperand::Incoming(_)) => {
                        return self.updated_value_retention(&source, capture, &paths);
                    }
                }
            }
            let Some(roots) = captured_lifetime_roots(self.program, &self.context.control, &value)
            else {
                return Handle::invalid();
            };
            let stored_value = self.context.control.operands.insert(value);
            self.insert_retention(roots, stored_value)
        }

        fn updated_value_retention(
            &mut self,
            source: &CanonicalPlace,
            capture: &FlowOperandFact,
            paths: &[Vec<BorrowLoanOwnerSegment>],
        ) -> Handle<FlowOperandRetention> {
            let mut leaves = Vec::new();
            for path in paths {
                let path = crate::borrow::owner_place_segments(path);
                let mut place = source.clone();
                place.segments.extend_from_slice(&path);
                let retention = match at_snapshot(
                    self.program,
                    &self.context.control,
                    capture.storage_snapshot,
                    &place,
                    StoredObservation::Binding,
                ) {
                    Some(stored) => self.retention_from_stored_operand(stored),
                    None => match self.incoming_value_retention(&place, capture.storage_snapshot) {
                        Some(retention) => retention,
                        None => return Handle::invalid(),
                    },
                };
                if self.context.control.retained_roots(retention).is_none() {
                    return Handle::invalid();
                }
                let path = self
                    .context
                    .control
                    .operand_retained_paths
                    .insert_many(path);
                leaves.push(FlowOperandRetainedLeaf { path, retention });
            }
            let leaves = self
                .context
                .control
                .operand_retained_leaves
                .insert_many(leaves);
            self.context
                .control
                .operand_retentions
                .insert(FlowOperandRetention {
                    leaves,
                    ..Default::default()
                })
        }

        /*
            A live descriptor load reads the current slot after its operands, not
            a deferred projection of the enclosing storage loan's dependency root.
            Rejoin only the actual evaluated receiver addresses and the shared
            post-effect stored-reference inventory. Detached copies/constructions/
            selections keep their immutable completed capture graph instead.
            This lifetime profile grants no address, case, access or loan authority.
        */
        fn live_descriptor_projection_retention(
            &mut self,
            expression: ExpressionHandle,
            receiver: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<Handle<FlowOperandRetention>> {
            if expression == self.reference_slot_target
                || receiver.state_symbol != self.state.symbol
                || receiver.statement_index != self.statement_index
                || !matches!(receiver.completion, FlowOperandCompletion::Observation)
                || !receiver.reference_leaves.is_empty()
                || !self
                    .context
                    .expression_result_type_at(self.program, self.machine, self.state, expression)
                    .is_some_and(|reference| self.is_view_type(reference))
            {
                return None;
            }
            let places = self.captured_reference_places(receiver.referents);
            if places.is_empty() {
                return None;
            }
            let mut profiles = Vec::new();
            for place in places {
                let Some(selected) = place.projected(path) else {
                    return Some(Handle::invalid());
                };
                let Some(retention) = self.stored_retention_at_place(&selected.place) else {
                    return Some(Handle::invalid());
                };
                if self.context.control.retained_roots(retention).is_none() {
                    return Some(Handle::invalid());
                }
                crate::shared::collections::push_unique(&mut profiles, retention);
            }
            Some(self.join_retention_profiles(&profiles))
        }

        pub(super) fn capture_member_retention(
            &mut self,
            expression: ExpressionHandle,
            receiver: &FlowOperandFact,
        ) -> Option<Handle<FlowOperandRetention>> {
            let member = self.program.expression_table.member_node(expression)?;
            let symbol = effective_member_symbol(self.program, member.receiver, member);
            if !symbol.is_valid() {
                return Some(Handle::invalid());
            }
            let mut path = Vec::new();
            if let Some(variant) = payload_variant_for_field(self.program, symbol) {
                path.push(PlaceSegment::Case { variant });
            }
            path.push(PlaceSegment::Field { symbol });
            if self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference))
            {
                let value = self.context.control.operands.insert(*receiver);
                let selected = validation::completed_storage::reference_sources_at_path(
                    self.program,
                    &self.context.control,
                    value,
                    &path,
                    self.storage_snapshot,
                );
                return Some(self.retention_from_stored_operand(selected));
            }
            let captured = self.captured_retention_at_path(*receiver, &path);
            if captured.is_valid() {
                return Some(captured);
            }
            if !receiver.constructed_children().is_empty()
                || (self
                    .context
                    .control
                    .operand_retentions
                    .is_valid(receiver.retention)
                    && {
                        let profile = self
                            .context
                            .control
                            .operand_retentions
                            .get(receiver.retention);
                        !profile.leaves.is_empty()
                            || !profile.alternatives.is_empty()
                            || self.context.control.operands.is_valid(profile.stored_value)
                            || self
                                .context
                                .control
                                .operand_retained_roots
                                .span_or_empty(profile.roots)
                                .iter()
                                .any(|root| root.as_expression().is_some())
                    })
            {
                return Some(self.projected_value_retention(*receiver, &path));
            }
            let places = self.captured_referent_places(receiver.referents);
            let mut roots = Vec::new();
            for mut place in places {
                place.segments.extend_from_slice(&path);
                let retention = self.stored_retention_at_place(&place)?;
                let Some(retained) = self.context.control.retained_roots(retention) else {
                    return Some(Handle::invalid());
                };
                for root in retained {
                    if !roots.contains(root) {
                        roots.push(*root);
                    }
                }
            }
            (!receiver.referents.is_empty()).then(|| self.append_retention(&roots))
        }

        fn returned_referent_roots(
            &mut self,
            actual: &FlowOperandFact,
            path: &[PlaceSegment],
        ) -> Option<Vec<PlaceRoot>> {
            let capture = self.captured_retention_at_path(*actual, path);
            self.context
                .control
                .retained_roots(capture)
                .map(<[_]>::to_vec)
        }

        fn returned_stored_roots(
            &mut self,
            sources: HandleSpan<checked_trees::FlowOperandStoredSource>,
        ) -> Option<Vec<PlaceRoot>> {
            let sources = self
                .context
                .control
                .operand_stored_sources
                .span(sources)?
                .to_vec();
            let mut roots = Vec::new();
            for source in sources {
                let selected = match source {
                    checked_trees::FlowOperandStoredSource::Value {
                        operand,
                        projection,
                        ..
                    } => {
                        let path = self
                            .context
                            .control
                            .operand_retained_paths
                            .span(projection)?
                            .to_vec();
                        let actual = *self.context.control.operands.get(operand);
                        self.returned_referent_roots(&actual, &path)?
                    }
                    checked_trees::FlowOperandStoredSource::Incoming { slot, .. } => {
                        let place = validation::CanonicalPlace {
                            root: slot.root,
                            segments: self
                                .context
                                .control
                                .operand_referent_segments
                                .span(slot.segments)?
                                .to_vec(),
                        };
                        if !self.is_incoming_descriptor_place(&place)
                            || !place.root.as_symbol().is_some_and(|symbol| {
                                self.program
                                    .state_parameter_by_symbol(self.state, symbol)
                                    .is_some()
                            })
                        {
                            return None;
                        }
                        vec![place.root]
                    }
                };
                for root in selected {
                    crate::shared::collections::push_unique(&mut roots, root);
                }
            }
            Some(roots)
        }

        /*
            Loading an indexed descriptor retains its pointee, not its array slot.
            A nonconstant index keeps every possible captured element lifetime.
        */
        pub(super) fn capture_indexed_retention(
            &mut self,
            expression: ExpressionHandle,
            collection: &FlowOperandFact,
            index: &FlowOperandFact,
            selector_writes_start: usize,
        ) -> Handle<FlowOperandRetention> {
            if !value_element_has_builtin_coordinate(
                self.program,
                self.machine,
                Some(self.state),
                expression,
                collection.type_reference,
            ) {
                return Handle::invalid();
            }
            let Some(later) = self.operand_writes.get(selector_writes_start..) else {
                return Handle::invalid();
            };
            let places = self.captured_referent_places(collection.referents);
            if later.iter().any(|mutation| {
                let Some(writes) = &mutation.bindings else {
                    return true;
                };
                writes.iter().any(|write| {
                    places.iter().any(|place| {
                        place.root == write.root
                            && canonical_place_segments_may_overlap(
                                self.program,
                                &place.segments,
                                &write.segments,
                            )
                    })
                })
            }) {
                return Handle::invalid();
            }
            let mut paths = Vec::new();
            if let Some(index) = self.captured_coordinate(index) {
                let path = [PlaceSegment::FixedIndex { index }];
                if let Some(retention) =
                    self.live_descriptor_projection_retention(expression, collection, &path)
                {
                    return retention;
                }
                let captured = self.captured_retention_at_path(*(collection), &path);
                return if captured.is_valid() {
                    captured
                } else {
                    self.projected_value_retention(*collection, &path)
                };
            } else {
                let mut profile = collection;
                if self
                    .context
                    .control
                    .operand_retentions
                    .is_valid(collection.retention)
                {
                    let stored = self
                        .context
                        .control
                        .operand_retentions
                        .get(collection.retention)
                        .stored_value;
                    if self.context.control.operands.is_valid(stored) {
                        profile = self.context.control.operands.get(stored);
                    }
                }
                let Some(leaves) =
                    borrow_carrying_owner_paths(self.program, profile.type_reference)
                else {
                    return Handle::invalid();
                };
                for leaf in leaves {
                    let path = crate::borrow::owner_place_segments(&leaf);
                    if let Some(PlaceSegment::FixedIndex { index }) = path.first() {
                        let path = vec![PlaceSegment::FixedIndex { index: *index }];
                        crate::shared::collections::push_unique(&mut paths, path);
                    }
                }
            }
            if paths.is_empty() {
                return Handle::invalid();
            }
            let Some(reference) = self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                expression,
            ) else {
                return Handle::invalid();
            };
            let mut profiles = Vec::new();
            let mut all_live = true;
            for path in &paths {
                match self.live_descriptor_projection_retention(expression, collection, path) {
                    Some(retention) if self.context.control.retained_roots(retention).is_some() => {
                        crate::shared::collections::push_unique(&mut profiles, retention);
                    }
                    Some(_) => return Handle::invalid(),
                    None => all_live = false,
                }
            }
            if all_live {
                return self.join_retention_profiles(&profiles);
            }
            if !profiles.is_empty() {
                return Handle::invalid();
            }
            let selections = paths
                .into_iter()
                .map(|path| (*collection, path))
                .collect::<Vec<_>>();
            self.capture_retention_join(reference, &selections)
        }

        pub(super) fn capture_returned_retention(
            &mut self,
            invocation: Handle<checked_trees::FlowCallFact>,
            expression: ExpressionHandle,
            destination: TypeReferenceHandle,
            call: &TableCallExpression,
            inputs: &[(ExpressionHandle, FlowOperandFact)],
            retention: &mut Option<Handle<FlowOperandRetention>>,
            source_snapshot: Handle<FlowStorageSnapshot>,
        ) {
            let Some(inputs) = inputs.get(usize::from(call.receiver.is_valid())..) else {
                *retention = Some(Handle::invalid());
                return;
            };
            let target = call_target_symbol(self.program, call);
            let call_ordinal =
                self.borrow
                    .with_state_borrows(self.machine.symbol, self.state.symbol, |rows| {
                        let [row] = rows else {
                            return None;
                        };
                        let mut calls = self
                            .borrow
                            .calls
                            .span(self.borrow.states.get(*row).calls)?
                            .iter()
                            .filter(|recorded| {
                                recorded.statement_index == self.statement_index
                                    && recorded.authored_expression == expression
                                    && recorded.target_symbol
                                        == call_target_entry_symbol(self.program, target)
                            });
                        let ordinal = calls.next()?.call_ordinal;
                        calls.next().is_none().then_some(ordinal)
                    });
            let Some(returned) = destination
                .is_valid()
                .then_some(destination)
                .or_else(|| {
                    self.context.expression_result_type_at(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                    )
                })
                .or_else(|| call_target_return_type(self.program, target))
            else {
                return;
            };
            if self.is_view_type(returned) {
                *retention = Some(Handle::invalid());
            }
            let Some(sources) = crate::borrow::call_reference_lifetime_sources(
                self.program,
                self.state.symbol,
                call,
            ) else {
                return;
            };
            let mut outputs: Vec<(
                Vec<PlaceSegment>,
                Vec<PlaceRoot>,
                bool,
                Option<Vec<FlowOperandRetainedSupplier>>,
            )> = Vec::new();
            for source in sources {
                let Some(path) =
                    crate::borrow::structural_owner_path(self.program, &source.owner_path)
                        .and_then(|path| loan_place_segments(self.program, returned, &path))
                else {
                    *retention = Some(Handle::invalid());
                    return;
                };
                let position = outputs
                    .iter()
                    .position(|(candidate, _, _, _)| *candidate == path)
                    .unwrap_or_else(|| {
                        outputs.push((path.clone(), Vec::new(), true, Some(Vec::new())));
                        outputs.len() - 1
                    });
                let Some((_, actual)) = inputs.get(source.parameter_index) else {
                    outputs[position].2 = false;
                    continue;
                };
                let source_path = crate::borrow::owner_place_segments(&source.source_path);
                let input = self
                    .context
                    .control
                    .calls
                    .get(invocation)
                    .argument_operands()
                    .handles()
                    .nth(source.parameter_index);
                let stored_sources = input.and_then(|input| {
                    if source.source == ViewReturnInputSource::Stored {
                        self.capture_stored_lifetime_sources(input, &source_path, source_snapshot)
                    } else {
                        Some(HandleSpan::empty())
                    }
                });
                let roots = if source.source == ViewReturnInputSource::Stored {
                    stored_sources.and_then(|sources| self.returned_stored_roots(sources))
                } else {
                    self.returned_referent_roots(actual, &source_path)
                };
                let Some(roots) = roots else {
                    outputs[position].2 = false;
                    continue;
                };
                for root in roots {
                    crate::shared::collections::push_unique(&mut outputs[position].1, root);
                }
                if let (Some(input), Some(stored_sources), Some(suppliers), Some(call_ordinal)) = (
                    input,
                    stored_sources,
                    &mut outputs[position].3,
                    call_ordinal,
                ) {
                    let source_path = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(source_path.iter().copied());
                    let projection = source_path;
                    let owner_path = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(crate::borrow::owner_place_segments(&source.owner_path));
                    suppliers.push(FlowOperandRetainedSupplier {
                        operand: input,
                        projection,
                        contract: Some(FlowOperandLifetimeContract {
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                            invocation: expression,
                            call_ordinal,
                            output_parameter: None,
                            input,
                            parameter_index: source.parameter_index,
                            source: source.source,
                            source_type: source.source_type,
                            source_path,
                            owner_path,
                            stored_sources,
                        }),
                    });
                } else {
                    outputs[position].3 = None;
                }
            }
            if let Some(images) = self.returned_image_paths(call, returned) {
                for path in images {
                    if !outputs
                        .iter()
                        .any(|(candidate, _, _, _)| *candidate == path)
                    {
                        outputs.push((path, Vec::new(), true, Some(Vec::new())));
                    }
                }
            }
            let mut leaves = Vec::new();
            for (path, roots, complete, suppliers) in outputs {
                let captured = if complete {
                    self.append_retention(&roots)
                } else {
                    Handle::invalid()
                };
                let descriptor = self.capture_returned_descriptor(
                    invocation,
                    call,
                    inputs,
                    returned,
                    &path,
                    captured,
                    source_snapshot,
                );
                let captured = if self.context.control.operand_retentions.is_valid(captured) {
                    let profile = *self.context.control.operand_retentions.get(captured);
                    let lifetime_sources = suppliers.map(|suppliers| {
                        self.context
                            .control
                            .operand_retained_suppliers
                            .insert_many(suppliers)
                    });
                    self.context
                        .control
                        .operand_retentions
                        .insert(FlowOperandRetention {
                            descriptor,
                            lifetime_sources,
                            ..profile
                        })
                } else {
                    captured
                };
                if path.is_empty() {
                    *retention = Some(captured);
                } else {
                    let path = self
                        .context
                        .control
                        .operand_retained_paths
                        .insert_many(path);
                    leaves.push(FlowOperandRetainedLeaf {
                        path,
                        retention: captured,
                    });
                }
            }
            if !leaves.is_empty() {
                let root_profile = if let Some(root) = *retention {
                    if !self.context.control.operand_retentions.is_valid(root) {
                        return;
                    }
                    *self.context.control.operand_retentions.get(root)
                } else {
                    FlowOperandRetention::default()
                };
                let leaves = self
                    .context
                    .control
                    .operand_retained_leaves
                    .insert_many(leaves);
                *retention = Some(self.context.control.operand_retentions.insert(
                    FlowOperandRetention {
                        leaves,
                        ..root_profile
                    },
                ));
            }
        }
    }
}

struct Invocation<'a> {
    call: &'a BorrowCallFact,
    site: InvocationSite,
    visited: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadPurpose {
    Value,
    Address,
}

struct EvaluatedOperand {
    boolean: Option<bool>,
    boolean_completion: Handle<boolean_completions::BooleanCompletions>,
    capture: checked_trees::FlowOperandFact,
}

#[derive(Clone, Copy, Default)]
struct CompletedValue {
    invocation: Handle<checked_trees::FlowCallFact>,
    boolean_completion: Handle<boolean_completions::BooleanCompletions>,
    /* Exact source payload read associated with this completion. */
    case_access: Handle<checked_trees::FlowCaseAccessFact>,
    /*
        A carrier actually formed by contextual value construction. This
        distinguishes an existing view from lending an owned operand; a
        receiving annotation alone does not make that judgment.
    */
    formed_type: TypeReferenceHandle,
    completion: checked_trees::FlowOperandCompletion,
    value_source: Handle<checked_trees::FlowOperandFact>,
    /*
        An address projection depends on its evaluated receiver's backing,
        independently of the projected value's reference leaves. Scalar value
        snapshots do not consume this place-only relationship.
    */
    backing: Handle<checked_trees::FlowOperandFact>,
    address_projection: Option<checked_trees::FlowOperandAddressProjection>,
    evaluated_inputs: HandleSpan<checked_trees::FlowOperandFact>,
    conditional_inputs_from: Option<usize>,
    referents: Option<HandleSpan<checked_trees::FlowOperandReferent>>,
    possible_storage: Option<HandleSpan<checked_trees::FlowOperandReferent>>,
    retention: Option<Handle<checked_trees::FlowOperandRetention>>,
    /* Descriptor contents substituted after operands, before callee writes. */
    reference_leaves: Option<HandleSpan<checked_trees::FlowCapturedReferenceLeaf>>,
}

/*
    The evaluator retains the full ordered roster. The call's receiver identity
    separates its explicit argument suffix from a namespace/receiver observation;
    statement calls and transitions already supply arguments only.
*/
struct InvocationInputs<'evaluated> {
    operands: &'evaluated [checked_trees::FlowOperandFact],
}

#[derive(Default)]
struct CompletedInvocation {
    call: Handle<checked_trees::FlowCallFact>,
    stores: Vec<stored_call_values::CallStore>,
    operands: HandleSpan<checked_trees::FlowOperandFact>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InvocationSite {
    Expression(ExpressionHandle),
    Statement,
    Transition(TransitionTargetHandle),
}

/*
    A branch restores the write timeline that reached its guard, retaining no
    effects from the sibling target. Arena rows are immutable; spans name prefixes.
*/
#[derive(Clone, Copy)]
pub(in crate::flow) struct EvaluatedWritePrefix {
    length: usize,
    pub(in crate::flow) storage_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
    pub(in crate::flow) writes: HandleSpan<checked_trees::FlowOperandReferent>,
    pub(in crate::flow) unknown: bool,
    pub(in crate::flow) binding_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    pub(in crate::flow) binding_unknown: bool,
}

#[derive(Clone)]
struct OperandMutation {
    observations: Option<Vec<CanonicalPlace>>,
    bindings: Option<Vec<CanonicalPlace>>,
}

pub(super) struct Execution<'a, 'b, 'plans> {
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &'a BorrowFacts,
    proof: &'a ProofFacts,
    domains: &'a DomainFacts,
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
    statement_index: usize,
    statement_fact: Handle<checked_trees::FlowStatementFact>,
    pub(super) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(super) semantic: &'b mut FactPlan,
    pub(super) context: &'b mut FlowBuildContext<'plans>,
    state_calls: &'b mut HandleSpan<FlowCallFact>,
    invocations: Vec<Invocation<'a>>,
    operand_writes: Vec<OperandMutation>,
    boolean_completions: arena::Arena<boolean_completions::BooleanCompletions>,
    entry_origin_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    entry_origin_unknown_write: bool,
    storage_snapshot: Handle<checked_trees::FlowStorageSnapshot>,
    entry_binding_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    entry_binding_unknown_write: bool,
    /*
        Only the final destination of a descriptor replacement names its slot.
        Intermediate view projections still load their original referent.
    */
    reference_slot_target: ExpressionHandle,
    /*
        Actual completed dispatch subjects in lexical evaluation scope. Alias
        reads borrow these captures; they never replay their source operands.
    */
    match_subjects: Vec<(
        Handle<typed_trees::expression::CompletedValueCapture>,
        checked_trees::FlowOperandFact,
    )>,
    read_purpose: ReadPurpose,
}

pub(super) struct StatementEvaluation {
    pub(super) stored_value: Handle<checked_trees::FlowOperandFact>,
    pub(super) store_target: Handle<checked_trees::FlowOperandFact>,
    pub(super) writes: EvaluatedWritePrefix,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_statement_calls<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    context: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement_fact: Handle<checked_trees::FlowStatementFact>,
    statement: &StatementNode,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) -> StatementEvaluation {
    let mut execution = Execution::new(
        program,
        borrow,
        proof,
        domains,
        machine,
        state,
        statement_index,
        statement_fact,
        semantic,
        context,
        state_calls,
        calls,
        contexts,
        constraints,
    );
    let mut stored_value = Handle::invalid();
    let mut store_target = Handle::invalid();
    match statement {
        StatementNode::Assembly(_) => {}
        StatementNode::Assignment(assignment) => {
            let mut owned_frames = None;
            let replaces_reference = super::shared_call_frames_or(
                execution.context.call_frames,
                program,
                &mut owned_frames,
            )
            .and_then(|frames| frames.assignment_replaces_reference_binding(machine, statement));
            if replaces_reference == Some(true) {
                execution.reference_slot_target = assignment.target;
            }
            let first_write = execution.operand_writes.len();
            let mut completed = CompletedValue::default();
            execution.read_purpose = ReadPurpose::Address;
            execution.evaluate_expression(
                assignment.target,
                TypeReferenceHandle::invalid(),
                contexts,
                constraints,
                &mut completed,
            );
            execution.read_purpose = ReadPurpose::Value;
            if replaces_reference.is_some() {
                let captured = execution.capture_place_operand(
                    assignment.target,
                    first_write,
                    completed,
                    *contexts,
                    constraints,
                );
                store_target = execution.context.control.operands.append(captured);
            }
            execution.reference_slot_target = ExpressionHandle::invalid();
            let destination =
                validation::assignment_stored_value_type(program, machine, state, assignment)
                    .unwrap_or_default();
            let evaluated = execution.evaluated_operand_as(
                assignment.value,
                destination,
                contexts,
                constraints,
            );
            stored_value = execution.context.control.operands.append(evaluated.capture);
        }
        StatementNode::LocalData(local) if !local.role.has_ordinary_initializer() => {}
        StatementNode::LocalData(local) => {
            let evaluated = execution.evaluated_operand_as(
                local.initial_value,
                local.type_reference,
                contexts,
                constraints,
            );
            stored_value = execution.context.control.operands.append(evaluated.capture);
        }
        StatementNode::Expression(expression) => {
            if state.return_type.is_valid()
                && statement_index + 1
                    == crate::flow::state_statements(program, state.statement_nodes).len()
            {
                let evaluated = execution.evaluated_operand_as(
                    *expression,
                    state.return_type,
                    contexts,
                    constraints,
                );
                if validation::expression_normal_completion_possible(program, *expression)
                    != Some(false)
                {
                    let returned_value =
                        execution.context.control.operands.append(evaluated.capture);
                    super::exits::append_outgoing_state_fact(
                        execution.context,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        TransitionTargetHandle::invalid(),
                        *constraints,
                        *constraints,
                        returned_value,
                        execution.storage_snapshot,
                        false,
                        HandleSpan::empty(),
                    );
                }
            } else {
                execution.expression(*expression, contexts, constraints);
            }
        }
        StatementNode::Call(call) => {
            let arguments = program.statement_table.expression_handles(call.arguments);
            let mut captured = vec![checked_trees::FlowOperandFact::default(); arguments.len()];
            for position in validation::CallSite::Statement(call)
                .argument_evaluation_order(program)
                .into_iter()
                .flatten()
            {
                let destination =
                    execution.invocation_parameter_type(InvocationSite::Statement, position);
                captured[position] = execution
                    .evaluated_operand_as(arguments[position], destination, contexts, constraints)
                    .capture;
            }
            let completed = execution.invoke(
                InvocationSite::Statement,
                InvocationInputs {
                    operands: &captured,
                },
                contexts,
                constraints,
            );
            let writes = super::operator_calls::operator_statement_call_mutated_places(
                program,
                execution.context,
                machine.symbol,
                state.symbol,
                statement_index,
                call,
            );
            if writes.as_ref().is_none_or(|writes| !writes.is_empty()) {
                execution.record_operand_writes(OperandMutation {
                    bindings: writes.clone(),
                    observations: writes,
                });
            }
            execution.install_call_stores(completed.call, completed.stores);
        }
        StatementNode::AssemblyFact(_) | StatementNode::Transition(_) => {}
    }
    StatementEvaluation {
        stored_value,
        store_target,
        writes: execution.write_prefix(),
    }
}

impl<'a, 'b, 'plans> Execution<'a, 'b, 'plans> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        program: &'plans dyn ::validation::CheckingInput,
        borrow: &'a BorrowFacts,
        proof: &'a ProofFacts,
        domains: &'a DomainFacts,
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
        statement_index: usize,
        statement_fact: Handle<checked_trees::FlowStatementFact>,
        semantic: &'b mut FactPlan,
        context: &'b mut FlowBuildContext<'plans>,
        state_calls: &'b mut HandleSpan<FlowCallFact>,
        calls: &'a [BorrowCallFact],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> Self {
        let mut malformed = false;
        let invocations = calls
            .iter()
            .filter_map(|call| {
                let site = match super::calls::memoized_find_call_site(
                    program,
                    &mut *context,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call.call_ordinal,
                ) {
                    Some(CallSite::Expression {
                        expression,
                        call: source,
                    }) => {
                        let (receiver, path) =
                            validation::call_receiver_parts(program, source.receiver);
                        let selected_target = validation::call_target_symbol(program, source);
                        let target = validation::resolve_state_call_target(
                            program,
                            machine,
                            state,
                            receiver,
                            selected_target,
                            path.as_deref(),
                            &source.target,
                        );
                        if call.receiver_symbol != receiver
                            || call.target_symbol
                                != if target.is_valid() {
                                    target
                                } else {
                                    source.target_symbol
                                }
                            || !program.expression_table.expression_is_valid(expression)
                        {
                            malformed = true;
                            return None;
                        }
                        InvocationSite::Expression(expression)
                    }
                    Some(CallSite::Statement(_)) => InvocationSite::Statement,
                    Some(CallSite::TransitionNamed { .. }) => {
                        let Some(target) = context
                            .transition_call_target_at(
                                program,
                                machine,
                                state,
                                statement_index,
                                call.call_ordinal,
                            )
                            .filter(|target| target.is_valid())
                        else {
                            malformed = true;
                            return None;
                        };
                        InvocationSite::Transition(target)
                    }
                    _ => {
                        malformed = true;
                        return None;
                    }
                };
                Some(Invocation {
                    call,
                    site,
                    visited: false,
                })
            })
            .collect();
        if malformed {
            *contexts = HandleSpan::empty();
            *constraints = HandleSpan::empty();
        }
        let storage_snapshot = context
            .control
            .statements
            .get(statement_fact)
            .storage_snapshot;
        Self {
            program,
            borrow,
            proof,
            domains,
            machine,
            state,
            statement_index,
            statement_fact,
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            semantic,
            context,
            state_calls,
            invocations,
            operand_writes: Vec::new(),
            boolean_completions: arena::Arena::default(),
            entry_origin_writes: HandleSpan::empty(),
            entry_origin_unknown_write: false,
            storage_snapshot,
            entry_binding_writes: HandleSpan::empty(),
            entry_binding_unknown_write: false,
            reference_slot_target: ExpressionHandle::invalid(),
            match_subjects: Vec::new(),
            read_purpose: ReadPurpose::Value,
        }
    }

    fn retain_value_observation(
        &mut self,
        expression: ExpressionHandle,
        receiver: Handle<checked_trees::FlowOperandFact>,
        places: HandleSpan<checked_trees::FlowOperandReferent>,
        semantic_contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: HandleSpan<FlowConstraintRef>,
    ) {
        let value = *self.context.control.operands.get(receiver);
        self.retain_copy_use(expression, &value);
        let control = &mut self.context.control;
        let statement = control.statements.get_mut(self.statement_fact);
        control.value_observations.append_to_span(
            &mut statement.value_observations,
            checked_trees::FlowValueObservationFact {
                expression,
                evaluation: self.evaluation,
                receiver,
                result: Handle::invalid(),
                places,
                origin_writes: self.entry_origin_writes,
                origin_unknown_write: self.entry_origin_unknown_write,
                semantic_contexts,
                constraints,
            },
        );
    }

    fn invoke(
        &mut self,
        site: InvocationSite,
        inputs: InvocationInputs<'_>,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> CompletedInvocation {
        let Some(invocation) = self
            .invocations
            .iter_mut()
            .find(|invocation| !invocation.visited && invocation.site == site)
        else {
            return CompletedInvocation::default();
        };
        invocation.visited = true;
        let captured_operands = inputs.operands;
        let borrowed_call = invocation.call;
        let writes = super::call_phases::call_storage_writes(
            self.program,
            self.borrow,
            self.context,
            self.machine,
            self.state,
            borrowed_call,
        );
        let built = build_call_flow_fact(
            self.program,
            self.borrow,
            self.proof,
            self.semantic,
            self.domains,
            self.context,
            self.machine,
            self.state,
            contexts,
            constraints,
            borrowed_call,
            captured_operands,
            writes.as_deref(),
        );
        let mut call = built.call;
        let writes = built.storage_writes;
        call.authored_expression = match site {
            InvocationSite::Expression(expression) => expression,
            InvocationSite::Statement | InvocationSite::Transition(_) => {
                ExpressionHandle::invalid()
            }
        };
        call.authored_receiver = match site {
            InvocationSite::Expression(expression) => {
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => call.receiver,
                    _ => ExpressionHandle::invalid(),
                }
            }
            InvocationSite::Statement | InvocationSite::Transition(_) => {
                ExpressionHandle::invalid()
            }
        };
        call.invocation_storage = self.storage_snapshot;
        call.operands = self
            .context
            .control
            .operands
            .insert_many(captured_operands.iter().copied());
        let argument_operands = call.argument_operands();

        let binding_writes = writes
            .as_ref()
            .and_then(|writes| writes.descriptor_stores());
        let stores =
            self.capture_call_stores(site, borrowed_call, argument_operands, binding_writes);
        super::call_phases::install_returned_reference_custody(
            self.program,
            self.borrow,
            self.context,
            self.machine.symbol,
            self.state.symbol,
            borrowed_call,
            binding_writes,
            constraints,
        );
        let observations = call
            .normal_return_writes_complete
            .then(|| {
                self.context
                    .control
                    .operand_origin_writes
                    .span(call.normal_return_writes)?
                    .iter()
                    .map(|write| {
                        Some(CanonicalPlace {
                            root: write.root,
                            segments: self
                                .context
                                .control
                                .operand_referent_segments
                                .span(write.segments)?
                                .to_vec(),
                        })
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .flatten();
        if ::validation::trace_flags::call_write_trace() {
            eprintln!(
                "CALLWRITE recorded state={:?} statement={} ordinal={} bindings={binding_writes:?} observations={observations:?}",
                self.state.symbol, borrowed_call.statement_index, borrowed_call.call_ordinal
            );
        }
        self.record_operand_writes(OperandMutation {
            bindings: binding_writes.map(<[_]>::to_vec),
            observations,
        });
        call.exit_semantic_contexts = *contexts;
        call.exit_constraints = *constraints;
        let operands = call.operands;
        let call = self
            .context
            .control
            .calls
            .append_to_span(self.state_calls, call);
        CompletedInvocation {
            call,
            stores,
            operands,
        }
    }

    pub(super) fn expression(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> Option<bool> {
        self.expression_result(
            expression,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        )
        .0
    }

    fn expression_result(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> (Option<bool>, CompletedValue) {
        let mut completed = CompletedValue::default();
        let value = self.evaluate_expression(
            expression,
            destination,
            contexts,
            constraints,
            &mut completed,
        );
        if !completed.invocation.is_valid() || !self.operand_is_view(expression) {
            self.append_result_facts(expression, contexts, constraints);
        }
        (value, completed)
    }

    /*
        Capture the completed evaluation itself, including nested constructed
        children. No expression-key lookup can substitute another evaluation.
    */
    fn evaluated_operand(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> EvaluatedOperand {
        self.evaluated_operand_as(
            expression,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        )
    }

    fn evaluated_operand_as(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> EvaluatedOperand {
        let previous_purpose = self.read_purpose;
        let source_is_view = self.operand_is_view(expression);
        self.read_purpose =
            if destination.is_valid() && self.is_view_type(destination) && !source_is_view {
                ReadPurpose::Address
            } else {
                ReadPurpose::Value
            };
        if let Some(mut capture) = self.dispatch_copy_value(expression, *constraints)
            && matches!(capture.completion, checked_trees::FlowOperandCompletion::Copied { path, .. } if path.is_empty())
        {
            self.retain_copy_use(expression, &capture);
            if self.is_view_type(capture.type_reference) {
                capture.referents = self.complete_reference_formation(
                    expression,
                    *constraints,
                    capture.referents,
                    capture.case_access,
                );
            } else {
                capture.referents = HandleSpan::empty();
            }
            self.read_purpose = previous_purpose;
            return EvaluatedOperand {
                boolean: None,
                boolean_completion: Handle::invalid(),
                capture,
            };
        }
        let first_write = self.operand_writes.len();
        let (boolean, completed) =
            self.expression_result(expression, destination, contexts, constraints);
        if let Some(mut capture) = self.dispatch_copy_value(expression, *constraints) {
            self.retain_copy_use(expression, &capture);
            if self.is_view_type(capture.type_reference) {
                capture.referents = self.complete_reference_formation(
                    expression,
                    *constraints,
                    capture.referents,
                    capture.case_access,
                );
            } else {
                capture.referents = HandleSpan::empty();
            }
            self.read_purpose = previous_purpose;
            return EvaluatedOperand {
                boolean,
                boolean_completion: completed.boolean_completion,
                capture,
            };
        }
        self.read_purpose = previous_purpose;
        let binds_view = if destination.is_valid() {
            self.is_view_type(destination)
        } else {
            self.operand_is_view(expression)
        };
        let produces_view = if completed.formed_type.is_valid() {
            self.is_view_type(completed.formed_type)
        } else {
            self.operand_is_view(expression)
        };
        if binds_view && !produces_view {
            let mut capture = self.capture_place_operand(
                expression,
                first_write,
                completed,
                *contexts,
                constraints,
            );
            capture.retention = self.formed_view_retention(&capture);
            capture.completion = checked_trees::FlowOperandCompletion::Observation;
            capture.value_source = Handle::invalid();
            capture.type_reference = destination;
            self.retain_observation_result(expression, &mut capture);
            return EvaluatedOperand {
                boolean,
                boolean_completion: completed.boolean_completion,
                capture,
            };
        }
        let observed_descriptor = if source_is_view && !binds_view && produces_view {
            let descriptor = self.capture_place_operand(
                expression,
                first_write,
                completed,
                *contexts,
                constraints,
            );
            self.context.control.operands.append(descriptor)
        } else {
            Handle::invalid()
        };
        let referents = if binds_view {
            completed
                .referents
                .unwrap_or_else(|| self.reference_operand_referents(expression))
        } else {
            HandleSpan::empty()
        };
        let referents = if binds_view {
            self.complete_reference_formation(
                expression,
                *constraints,
                referents,
                completed.case_access,
            )
        } else {
            referents
        };
        let mut capture =
            self.capture_operand(expression, first_write, *contexts, *constraints, referents);
        capture.case_access = completed.case_access;
        capture.completion = completed.completion;
        capture.value_source = completed.value_source;
        capture.evaluated_inputs = completed.evaluated_inputs;
        capture.conditional_inputs_from = completed.conditional_inputs_from;
        capture.backing = if observed_descriptor.is_valid() {
            observed_descriptor
        } else {
            completed.backing
        };
        capture.address_projection = if observed_descriptor.is_valid() {
            None
        } else {
            completed.address_projection
        };
        capture.type_reference = if destination.is_valid() {
            destination
        } else {
            self.context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .unwrap_or_default()
        };
        let carries_borrows = !binds_view
            && validation::borrow_contracts::is_borrow_carrying_data(
                self.program,
                capture.type_reference,
            );
        if binds_view {
            if let Some(retention) =
                self.captured_reference_retention(&capture, completed.retention)
            {
                capture.retention = retention;
            }
        } else {
            capture.retention = if carries_borrows {
                completed
                    .retention
                    .unwrap_or_else(|| self.capture_value_retention(&capture))
            } else {
                Handle::invalid()
            };
        }
        if let Some(backing) = self.capture_invocation_backing(completed.invocation, &capture) {
            self.retain_possible_storage(&mut capture, &backing);
        }
        self.append_invocation_memberships(
            completed.invocation,
            &mut capture,
            contexts,
            constraints,
        );
        self.retain_invocation_permissions(completed.invocation, &capture);
        if !binds_view {
            let mut evaluated = EvaluatedOperand {
                boolean,
                boolean_completion: completed.boolean_completion,
                capture,
            };
            self.publish_completed_owned_value(&mut evaluated, contexts, constraints);
            capture = evaluated.capture;
        }
        if carries_borrows
            && matches!(
                capture.completion,
                checked_trees::FlowOperandCompletion::Observation
            )
        {
            if let Some(leaves) = completed.reference_leaves {
                capture.reference_leaves = self.publish_returned_reference_leaves(&capture, leaves);
                self.retain_observation_result(expression, &mut capture);
                return EvaluatedOperand {
                    boolean,
                    boolean_completion: completed.boolean_completion,
                    capture,
                };
            }
            let owned_places = if let Some(referents) = completed.referents {
                self.captured_reference_places(referents)
            } else {
                place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                )
                .into_iter()
                .map(super::CapturedReferencePlace::unformed)
                .collect()
            };
            let owner = self.context.control.operands.append(capture);
            capture.reference_leaves = self.capture_owned_reference_leaves(owner, &owned_places);
            capture.value_source = owner;
        }
        self.retain_descriptor_permission(&mut capture, constraints);
        self.retain_observation_result(expression, &mut capture);
        EvaluatedOperand {
            boolean,
            boolean_completion: completed.boolean_completion,
            capture,
        }
    }

    /*
        Attach the actual completed result, not a reconstructed expression or
        a type-only claim that the observer cannot retain its receiver.
    */
    fn retain_observation_result(
        &mut self,
        expression: ExpressionHandle,
        capture: &mut checked_trees::FlowOperandFact,
    ) {
        let control = &mut self.context.control;
        let observations = control
            .statements
            .get(self.statement_fact)
            .value_observations;
        let matching = observations
            .handles()
            .filter(|handle| {
                let observation = control.value_observations.get(*handle);
                observation.expression == expression
                    && observation.evaluation == self.evaluation
                    && !observation.result.is_valid()
            })
            .collect::<Vec<_>>();
        if let [observation] = matching.as_slice() {
            let result = control.operands.append(*capture);
            control.value_observations.get_mut(*observation).result = result;
            capture.value_source = result;
        }
    }

    /*
        Address formation observes a place, even when reading that place would
        produce an owned value. Keep an indexed child's completed coordinates;
        an unresolved produced address cannot be recovered from later syntax.
    */
    fn evaluated_place(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> EvaluatedOperand {
        if expression != self.reference_slot_target
            && let Some(mut capture) = self.dispatch_copy_value(expression, *constraints)
            && matches!(capture.completion, checked_trees::FlowOperandCompletion::Copied { path, .. } if path.is_empty())
        {
            if self.is_view_type(capture.type_reference) {
                capture.referents = self.complete_reference_formation(
                    expression,
                    *constraints,
                    capture.referents,
                    capture.case_access,
                );
            }
            self.retain_copy_use(expression, &capture);
            return EvaluatedOperand {
                boolean: None,
                boolean_completion: Handle::invalid(),
                capture,
            };
        }
        let first_write = self.operand_writes.len();
        let (boolean, completed) = self.expression_result(
            expression,
            TypeReferenceHandle::invalid(),
            contexts,
            constraints,
        );
        if expression != self.reference_slot_target
            && let Some(mut capture) = self.dispatch_copy_value(expression, *constraints)
        {
            if self.is_view_type(capture.type_reference) {
                capture.referents = self.complete_reference_formation(
                    expression,
                    *constraints,
                    capture.referents,
                    capture.case_access,
                );
            }
            self.retain_copy_use(expression, &capture);
            return EvaluatedOperand {
                boolean,
                boolean_completion: completed.boolean_completion,
                capture,
            };
        }
        let mut capture =
            self.capture_place_operand(expression, first_write, completed, *contexts, constraints);
        self.retain_observation_result(expression, &mut capture);
        EvaluatedOperand {
            boolean,
            boolean_completion: completed.boolean_completion,
            capture,
        }
    }

    /*
        Retain an already formed address. Unlike value evaluation, assignment
        capture publishes no result facts about the destination's old contents.
    */
    fn capture_place_operand(
        &mut self,
        expression: ExpressionHandle,
        first_write: usize,
        completed: CompletedValue,
        contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> checked_trees::FlowOperandFact {
        let reference = if completed.formed_type.is_valid() {
            completed.formed_type
        } else {
            self.context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .unwrap_or_default()
        };
        let produces_view = matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Borrow(_)
        ) || (reference.is_valid() && self.is_view_type(reference));
        let referents = completed.referents.unwrap_or_else(|| {
            if expression != self.reference_slot_target && produces_view {
                let referents = self.reference_operand_referents(expression);
                if !referents.is_empty() || !self.names_whole_parameter(expression) {
                    return referents;
                }
            }
            let Some(place) = place_at(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            ) else {
                return HandleSpan::empty();
            };
            if !reference.is_valid() && !matches!(place.root, facts::PlaceRoot::Expression(_)) {
                return HandleSpan::empty();
            }
            self.append_referent_places(&[place])
        });
        let referents = if expression != self.reference_slot_target && produces_view {
            self.complete_reference_formation(
                expression,
                *constraints,
                referents,
                completed.case_access,
            )
        } else {
            referents
        };
        let mut capture =
            self.capture_operand(expression, first_write, contexts, *constraints, referents);
        capture.case_access = completed.case_access;
        capture.completion = completed.completion;
        capture.value_source = completed.value_source;
        capture.evaluated_inputs = completed.evaluated_inputs;
        capture.conditional_inputs_from = completed.conditional_inputs_from;
        capture.backing = completed.backing;
        capture.address_projection = completed.address_projection;
        capture.type_reference = reference;
        if let Some(leaves) = completed.reference_leaves {
            capture.retention = completed.retention.unwrap_or_default();
            capture.reference_leaves = self.publish_returned_reference_leaves(&capture, leaves);
        }
        if expression != self.reference_slot_target && produces_view {
            if let Some(retention) =
                self.captured_reference_retention(&capture, completed.retention)
            {
                capture.retention = retention;
            }
        } else {
            capture.retention =
                self.capture_storage_retention(&capture, completed.retention, completed.backing);
        }
        if let Some(possible_storage) = completed.possible_storage
            && !possible_storage.is_empty()
            && self
                .context
                .control
                .operand_retentions
                .is_valid(capture.retention)
        {
            let profile = *self
                .context
                .control
                .operand_retentions
                .get(capture.retention);
            capture.retention = self.context.control.operand_retentions.insert(
                checked_trees::FlowOperandRetention {
                    possible_storage,
                    ..profile
                },
            );
        }
        if expression != self.reference_slot_target {
            self.retain_descriptor_permission(&mut capture, constraints);
        }
        capture
    }

    pub(in crate::flow) fn write_prefix(&self) -> EvaluatedWritePrefix {
        EvaluatedWritePrefix {
            length: self.operand_writes.len(),
            storage_snapshot: self.storage_snapshot,
            writes: self.entry_origin_writes,
            unknown: self.entry_origin_unknown_write,
            binding_writes: self.entry_binding_writes,
            binding_unknown: self.entry_binding_unknown_write,
        }
    }

    pub(in crate::flow) fn restore_write_prefix(&mut self, prefix: EvaluatedWritePrefix) {
        self.operand_writes.truncate(prefix.length);
        self.storage_snapshot = prefix.storage_snapshot;
        self.entry_origin_writes = prefix.writes;
        self.entry_origin_unknown_write = prefix.unknown;
        self.entry_binding_writes = prefix.binding_writes;
        self.entry_binding_unknown_write = prefix.binding_unknown;
    }

    fn record_operand_writes(&mut self, writes: OperandMutation) {
        for (places, retained, unknown) in [
            (
                &writes.observations,
                &mut self.entry_origin_writes,
                &mut self.entry_origin_unknown_write,
            ),
            (
                &writes.bindings,
                &mut self.entry_binding_writes,
                &mut self.entry_binding_unknown_write,
            ),
        ] {
            if let Some(places) = places {
                for place in places {
                    self.context.append_origin_write(retained, place);
                }
            } else {
                *unknown = true;
            }
        }
        self.storage_snapshot = super::stored_values::append_place_writes(
            &mut self.context.control,
            self.storage_snapshot,
            writes.bindings.as_deref(),
            writes.observations.as_deref(),
        );
        self.operand_writes.push(writes);
    }

    fn join_storage_prefixes(&mut self, prefixes: &[EvaluatedWritePrefix]) {
        let predecessors = prefixes
            .iter()
            .map(|prefix| prefix.storage_snapshot)
            .collect::<Vec<_>>();
        self.storage_snapshot =
            super::stored_values::join(&mut self.context.control, &predecessors);
    }

    fn capture_operand(
        &mut self,
        expression: ExpressionHandle,
        first_write: usize,
        contexts: HandleSpan<FlowSemanticContextRef>,
        constraints: HandleSpan<FlowConstraintRef>,
        referents: HandleSpan<checked_trees::FlowOperandReferent>,
    ) -> checked_trees::FlowOperandFact {
        let changed = self.changed_operand_sources(&[(expression, first_write)]);
        let mut captured_contexts = contexts;
        let mut captured_constraints = constraints;
        self.filter_captured_sources(&changed, &mut captured_contexts, &mut captured_constraints);
        let retention = self.retention_from_referents(referents);
        checked_trees::FlowOperandFact {
            descriptor_permission: Default::default(),
            expression,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
            evaluation: self.evaluation,
            type_reference: TypeReferenceHandle::invalid(),
            completion: checked_trees::FlowOperandCompletion::Observation,
            backing: Handle::invalid(),
            address_projection: None,
            evaluated_inputs: HandleSpan::empty(),
            conditional_inputs_from: None,
            value_source: Handle::invalid(),
            entry_origin_writes: self.entry_origin_writes,
            entry_origin_unknown_write: self.entry_origin_unknown_write,
            entry_binding_writes: self.entry_binding_writes,
            entry_binding_unknown_write: self.entry_binding_unknown_write,
            storage_snapshot: self.storage_snapshot,
            case_access: Handle::invalid(),
            constraints: captured_constraints,
            referents,
            retention,
            reference_leaves: HandleSpan::empty(),
        }
    }

    fn invocation_parameter_type(
        &mut self,
        site: InvocationSite,
        position: usize,
    ) -> TypeReferenceHandle {
        let ordinal = self
            .invocations
            .iter()
            .find(|invocation| !invocation.visited && invocation.site == site)
            .map(|invocation| invocation.call.call_ordinal);
        // An applied call can receive a constructed record where its declaration
        // names only a type parameter. Keep that receiving signature so the
        // completed operand retains the record's fields and their loan backing.
        ordinal
            .and_then(|ordinal| {
                super::calls::memoized_find_call_site(
                    self.program,
                    &mut *self.context,
                    self.machine.symbol,
                    self.state.symbol,
                    self.statement_index,
                    ordinal,
                )
            })
            .and_then(|site| site.parameters(self.program, self.machine.symbol))
            .and_then(|parameters| parameters.get(position))
            .map_or_else(TypeReferenceHandle::invalid, |parameter| {
                parameter.type_reference
            })
    }

    /*
        Contextual construction installs an owned value before an argument
        borrows it. A formal reference describes that later access, not the
        constructor's storage. Its retained result type supplies the exact
        nominal application and fixed extent without peeling reference shells.
        Operand capture still retains the original destination and its loans.
    */
    fn constructed_value_type(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        if destination.is_valid()
            && validation::reference_shell(self.program, destination).is_none()
        {
            Some(destination)
        } else {
            self.context.expression_result_type_at(
                self.program,
                self.machine,
                self.state,
                expression,
            )
        }
    }

    fn constructed_child_type(
        &mut self,
        parent: ExpressionHandle,
        destination: TypeReferenceHandle,
        path: &[facts::PlaceSegment],
    ) -> TypeReferenceHandle {
        let destination = self
            .constructed_value_type(parent, destination)
            .unwrap_or_default();
        if let Some(reference) = super::operand_values::operand_child_type(
            self.program,
            destination,
            path,
            TypeReferenceHandle::invalid(),
        ) {
            return reference;
        }
        let declared = self
            .context
            .expression_result_type_at(self.program, self.machine, self.state, parent)
            .and_then(|reference| {
                validation::project_type_reference_from_segments(self.program, reference, path)
            })
            .or_else(|| validation::expression_place_type_reference(self.program, parent, path))
            .unwrap_or_default();
        super::operand_values::operand_child_type(self.program, destination, path, declared)
            .unwrap_or_default()
    }

    fn append_operand_child(
        &mut self,
        parent: ExpressionHandle,
        path: &[facts::PlaceSegment],
        type_reference: TypeReferenceHandle,
        capture: checked_trees::FlowOperandFact,
        children: &mut HandleSpan<checked_trees::FlowOperandChild>,
    ) {
        let value = self.context.control.operands.append(capture);
        let path = self
            .context
            .control
            .operand_child_paths
            .insert_many(path.iter().copied());
        reference_spans::append_flow_reference(
            &mut self.context.control.operand_children,
            children,
            checked_trees::FlowOperandChild {
                parent,
                path,
                value,
                type_reference,
            },
        );
    }

    /*
        Exact storage this operand's evaluated value denotes at invocation when
        it is a view carrier. A `Borrow` operand evaluates to a reference; any
        other operand is a view when its result type unwraps to a reference or
        slice. Detached carriers read their operand-time copy, so only a view
        transports the referent's live facts. The fail-closed resolver returns
        `None` for referents it cannot place exactly; those operands keep the
        context-identity intersection transport.
    */
    fn operand_is_view(&mut self, expression: ExpressionHandle) -> bool {
        if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Borrow(_)
        ) {
            true
        } else {
            self.context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference))
        }
    }

    fn names_whole_parameter(&self, expression: ExpressionHandle) -> bool {
        place_at(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )
        .is_some_and(|place| {
            place.segments.is_empty()
                && place.root.as_symbol().is_some_and(|symbol| {
                    typed_trees::signature::state_parameter_position(
                        self.program.state_parameters(self.state),
                        symbol,
                    )
                    .is_some()
                })
        })
    }

    fn is_view_type(&self, reference: typed_trees::types::TypeReferenceHandle) -> bool {
        super::operand_contexts::operand_binds_live_view(self.program, reference)
    }

    fn reference_operand_referents(
        &mut self,
        expression: ExpressionHandle,
    ) -> HandleSpan<checked_trees::FlowOperandReferent> {
        if let Some(referents) = self.stored_view_referents(expression) {
            return referents;
        }
        let Some(referents) = crate::flow::reference_expression_storage_places_including_shared(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
            self.context.call_frames,
        ) else {
            return HandleSpan::empty();
        };
        let rows: Vec<checked_trees::FlowOperandReferent> = referents
            .iter()
            .map(|referent| checked_trees::FlowOperandReferent {
                formation: arena::Handle::invalid(),
                root: referent.root,
                segments: self
                    .context
                    .control
                    .operand_referent_segments
                    .insert_many(referent.segments.iter().copied()),
            })
            .collect();
        self.context
            .control
            .operand_referents
            .insert_many(rows.iter().copied())
    }

    fn record_operator_invocation(
        &mut self,
        expression: ExpressionHandle,
        occurrence: checked_trees::CheckedOperatorOccurrence,
        captured_operands: &[checked_trees::FlowOperandFact],
        constraints: HandleSpan<FlowConstraintRef>,
    ) {
        let mut operand_span = HandleSpan::empty();
        let machine = self.machine.symbol;
        let state = self.state.symbol;
        let statement = self.statement_index;
        let control = &mut self.context.control;
        self.context.operators.for_each_expression_use(expression, |operator_use, use_fact| {
            if use_fact.expression == expression
                && use_fact.occurrence == occurrence
                && use_fact.selected_operator_symbol.is_valid()
                && matches!(use_fact.origin, checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol, state_symbol, statement_index, ..
                } if machine_symbol == machine && state_symbol == state && statement_index == statement)
            {
                if operand_span.is_empty() {
                    operand_span = control.operands
                        .insert_many(captured_operands.iter().copied());
                }
                control.operator_invocations.append_to_span(
                    &mut control.statements.get_mut(self.statement_fact).operator_invocations,
                    checked_trees::FlowOperatorInvocationFact {
                        operator_use,
                        named_use: arena::Handle::invalid(),
                        operands: operand_span,
                        requires_constraints: constraints,
                    },
                );
            }
        });
    }

    /*
        Named `Namespace::requirement(...)` uses that name this call
        expression at this statement. Named uses produce no `uses` row, so
        their invocation evidence keys on the `named_use` handle instead.
    */
    fn named_uses_at(
        &self,
        expression: ExpressionHandle,
    ) -> Vec<arena::Handle<checked_trees::CheckedNamedOperatorUseFact>> {
        self.context
            .operators
            .named_uses
            .iter()
            .filter(|(_, named_use)| {
                named_use.expression == expression
                    && matches!(named_use.origin, checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol, state_symbol, statement_index, ..
                    } if machine_symbol == self.machine.symbol
                        && state_symbol == self.state.symbol
                        && statement_index == self.statement_index)
            })
            .map(|(handle, _)| handle)
            .collect()
    }

    /*
        Each matching `named_uses` row gets a capture naming that row. The
        caller already captured every evaluated position at its own
        completion — as a spelled use would — so a bound operand's row still
        carries the facts a later operand's write destroyed. Only
        parameter-bound operands are selected for the invocation: a static
        namespace receiver is a path, not an operand, mirroring
        `named_call_operands`.
    */
    fn record_named_operator_invocations(
        &mut self,
        call: &typed_trees::expression::TableCallExpression,
        named_uses: &[arena::Handle<checked_trees::CheckedNamedOperatorUseFact>],
        captured: &[(ExpressionHandle, checked_trees::FlowOperandFact)],
        constraints: HandleSpan<FlowConstraintRef>,
    ) {
        for &named_use in named_uses {
            let Some(operator) = typed_trees::operator::declaration_by_symbol(
                self.program,
                self.context
                    .operators
                    .named_uses
                    .get(named_use)
                    .selected_operator_symbol,
            ) else {
                continue;
            };
            let parameters = self.program.operator_parameters(operator);
            let Some(named_operands) =
                crate::published::facts::operator_crashes::named_call_operands(
                    self.program,
                    call,
                    parameters,
                )
            else {
                continue;
            };
            let mut cursor = 0;
            let mut bound = Vec::with_capacity(named_operands.len());
            for operand in &named_operands {
                let Some(offset) = captured[cursor..]
                    .iter()
                    .position(|(expression, _)| expression == operand)
                else {
                    break;
                };
                bound.push(captured[cursor + offset].1);
                cursor += offset + 1;
            }
            if bound.len() != named_operands.len() {
                continue;
            }
            let operand_span = self
                .context
                .control
                .operands
                .insert_many(bound.iter().copied());
            self.context.control.operator_invocations.append_to_span(
                &mut self
                    .context
                    .control
                    .statements
                    .get_mut(self.statement_fact)
                    .operator_invocations,
                checked_trees::FlowOperatorInvocationFact {
                    operator_use: arena::Handle::invalid(),
                    named_use,
                    operands: operand_span,
                    requires_constraints: constraints,
                },
            );
        }
    }

    fn evaluate_expression(
        &mut self,
        expression: ExpressionHandle,
        destination: TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
        completed: &mut CompletedValue,
    ) -> Option<bool> {
        let CompletedValue {
            invocation,
            boolean_completion,
            case_access,
            formed_type,
            completion,
            value_source,
            backing,
            address_projection,
            evaluated_inputs,
            conditional_inputs_from,
            referents,
            possible_storage,
            retention,
            reference_leaves,
        } = completed;
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        if let Some(receiver) = validation::collection_length_receiver(
            self.program,
            self.machine,
            Some(self.state),
            expression,
        ) {
            let receiver = self.evaluated_place(receiver, contexts, constraints);
            *backing = self.context.control.operands.insert(receiver.capture);
            *referents = Some(HandleSpan::empty());
            self.retain_value_observation(
                expression,
                *backing,
                HandleSpan::empty(),
                *contexts,
                *constraints,
            );
            return self.live_boolean(expression, *contexts);
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(saved) => {
                let captured = self
                    .match_subjects
                    .iter()
                    .rev()
                    .find_map(|(capture, value)| (*capture == saved.capture).then_some(*value));
                if let Some(captured) = captured {
                    *formed_type = captured.type_reference;
                    *completion = captured.completion;
                    let source = self.context.control.operands.insert(captured);
                    *backing = source;
                    if !self.is_view_type(captured.type_reference)
                        && validation::reference_shell(self.program, captured.type_reference)
                            .is_none()
                        && self.program.type_multiplicity(captured.type_reference)
                            != language_semantics::Multiplicity::Unrestricted
                    {
                        // This noncopy saved subject observes the same owned
                        // home as its actual captured supplier. The empty
                        // projection carries that role, not a fresh snapshot
                        // or permission to follow a stored reference.
                        *address_projection = Some(
                            checked_trees::FlowOperandAddressProjection::Place(HandleSpan::empty()),
                        );
                    }
                    if self.is_view_type(captured.type_reference)
                        || self.program.type_multiplicity(captured.type_reference)
                            == language_semantics::Multiplicity::Unrestricted
                    {
                        *value_source = source;
                    }
                    *referents = Some(if self.is_view_type(captured.type_reference) {
                        captured.referents
                    } else {
                        validation::ownership_place_from_expression_in_state(
                            self.program,
                            self.state.symbol,
                            self.statement_index,
                            expression,
                        )
                        .map(|place| self.append_referent_places(&[place]))
                        .unwrap_or_default()
                    });
                    *retention = Some(captured.retention);
                    *reference_leaves = Some(captured.reference_leaves);
                } else {
                    *referents = Some(HandleSpan::empty());
                    *retention = Some(Handle::invalid());
                }
                return None;
            }
            ExpressionNode::Match(dispatch) => {
                *formed_type = destination;
                let alternatives =
                    self.dispatch(expression, *dispatch, destination, contexts, constraints);
                *completion = checked_trees::FlowOperandCompletion::Selected(alternatives);
                let binds_view = if destination.is_valid() {
                    self.is_view_type(destination)
                } else {
                    self.operand_is_view(expression)
                };
                if binds_view {
                    *referents = Some(self.capture_alternative_referents(alternatives));
                }
                *retention = Some(self.capture_alternative_retention(expression, alternatives));
                return None;
            }
            ExpressionNode::Boolean(value) => return Some(*value),
            ExpressionNode::Name(_) => {
                *case_access = self.record_case_access(expression, *constraints);
                return self.live_boolean(expression, *contexts);
            }
            ExpressionNode::Member(member) => {
                let receiver = self.evaluated_place(member.receiver, contexts, constraints);
                *backing = self.context.control.operands.insert(receiver.capture);
                *referents = Some(self.capture_member_referents(
                    expression,
                    &receiver.capture,
                    *constraints,
                    case_access,
                    possible_storage,
                    address_projection,
                ));
                *retention = self.capture_member_retention(expression, &receiver.capture);
                self.retain_value_observation(
                    expression,
                    *backing,
                    referents.unwrap_or_default(),
                    *contexts,
                    *constraints,
                );
                return self.live_boolean(expression, *contexts);
            }
            ExpressionNode::Binary(binary) => {
                let case_observation = validation::applied_case_membership_variant(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                    binary,
                )
                .is_some();
                let evaluated_left = if case_observation {
                    self.evaluated_place(binary.left, contexts, constraints)
                } else {
                    self.evaluated_operand(binary.left, contexts, constraints)
                };
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let (value, completion, inputs, conditional_from) =
                        self.logical_expression(binary, evaluated_left, contexts, constraints);
                    *boolean_completion = completion;
                    *evaluated_inputs = inputs;
                    *conditional_inputs_from = conditional_from;
                    return value;
                }
                let left_operand = evaluated_left.capture;
                let right_operand = self
                    .evaluated_operand(binary.right, contexts, constraints)
                    .capture;
                if !case_observation {
                    *evaluated_inputs = self
                        .context
                        .control
                        .operands
                        .insert_many([left_operand, right_operand]);
                }
                self.record_operator_invocation(
                    expression,
                    checked_trees::CheckedOperatorOccurrence::Expression,
                    &[left_operand, right_operand],
                    *constraints,
                );
                if case_observation {
                    let receiver = self.context.control.operands.append(left_operand);
                    self.retain_value_observation(
                        expression,
                        receiver,
                        left_operand.referents,
                        *contexts,
                        *constraints,
                    );
                }
                return crate::values::evaluate_closed_boolean_expression(
                    self.program,
                    self.context.operators,
                    expression,
                    &[],
                );
            }
            ExpressionNode::Call(call)
                if (call.layout_query.is_some() || call.native_code_layout.is_some()) =>
            {
                return None;
            }
            ExpressionNode::Call(call)
                if validation::compiler_operations::storage_address_operand(self.program, call)
                    .is_some() =>
            {
                let operand =
                    validation::compiler_operations::storage_address_operand(self.program, call)?;
                let value = self.evaluated_operand(operand, contexts, constraints);
                let receiver = self.context.control.operands.insert(value.capture);
                self.retain_value_observation(
                    expression,
                    receiver,
                    value.capture.referents,
                    *contexts,
                    *constraints,
                );
                return None;
            }
            ExpressionNode::Call(call) => {
                let named_uses = self.named_uses_at(expression);
                let mut captured = Vec::new();
                if call.receiver.is_valid() {
                    let receiver = self.evaluated_operand(call.receiver, contexts, constraints);
                    captured.push((call.receiver, receiver.capture));
                }
                let prefix = captured.len();
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                captured.resize(
                    prefix + arguments.len(),
                    (
                        ExpressionHandle::invalid(),
                        checked_trees::FlowOperandFact::default(),
                    ),
                );
                for position in (validation::CallSite::Expression { expression, call })
                    .argument_evaluation_order(self.program)
                    .ok()?
                {
                    let argument = arguments[position];
                    let destination = self.invocation_parameter_type(
                        InvocationSite::Expression(expression),
                        position,
                    );
                    captured[prefix + position] = (
                        argument,
                        self.evaluated_operand_as(argument, destination, contexts, constraints)
                            .capture,
                    );
                }
                self.record_named_operator_invocations(call, &named_uses, &captured, *constraints);
                self.capture_returned_referents(expression, call, &captured, referents);
                *reference_leaves = self.capture_call_reference_leaves(
                    expression,
                    destination,
                    call,
                    &captured,
                    *constraints,
                );
                let source_snapshot = self.storage_snapshot;
                let operands = captured
                    .iter()
                    .map(|(_, operand)| *operand)
                    .collect::<Vec<_>>();
                let completed = self.invoke(
                    InvocationSite::Expression(expression),
                    InvocationInputs {
                        operands: &operands,
                    },
                    contexts,
                    constraints,
                );
                *invocation = completed.call;
                self.complete_call_reference_formations(
                    completed.call,
                    referents,
                    reference_leaves,
                );
                *evaluated_inputs = completed.operands;
                if let Some(variant) = returned_constructor_case(self.program, call) {
                    self.append_value_observation(
                        expression,
                        &[],
                        facts::FactPayload::CaseValue {
                            variant,
                            value: true,
                        },
                        contexts,
                        constraints,
                    );
                }
                for &named_use in &named_uses {
                    super::operator_calls::apply_named_operator_call_effects(
                        self.program,
                        self.domains,
                        self.semantic,
                        self.context,
                        self.machine.symbol,
                        self.state.symbol,
                        self.statement_index,
                        call,
                        named_use,
                        contexts,
                        constraints,
                    );
                    let writes = super::operator_calls::named_operator_call_mutated_places(
                        self.program,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        call,
                        named_use,
                    );
                    self.record_operand_writes(OperandMutation {
                        bindings: writes.clone(),
                        observations: writes,
                    });
                }
                self.install_call_stores(completed.call, completed.stores);
                self.capture_returned_retention(
                    completed.call,
                    expression,
                    destination,
                    call,
                    &captured,
                    retention,
                    source_snapshot,
                );
                self.capture_formation_enclosure(expression, call, &captured, retention);
            }
            ExpressionNode::ArrayLiteral(values) => {
                let mut children = HandleSpan::empty();
                for (index, value) in self
                    .program
                    .expression_table
                    .expression_handles(*values)
                    .iter()
                    .enumerate()
                {
                    let path = [facts::PlaceSegment::FixedIndex { index }];
                    let child_type = self.constructed_child_type(expression, destination, &path);
                    let evaluated =
                        self.evaluated_operand_as(*value, child_type, contexts, constraints);
                    self.append_owned_value_observations(
                        expression,
                        &path,
                        child_type,
                        &evaluated,
                        contexts,
                        constraints,
                    );
                    self.append_operand_child(
                        expression,
                        &path,
                        child_type,
                        evaluated.capture,
                        &mut children,
                    );
                }
                *completion = checked_trees::FlowOperandCompletion::Constructed(children);
            }
            ExpressionNode::StructLiteral(value) => {
                let mut children = HandleSpan::empty();
                let carrier =
                    self.constructed_value_type(expression, destination)
                        .map(|reference| {
                            validation::peel_declared_constraints(self.program, reference)
                        });
                for field in self.program.expression_table.struct_fields(value.fields) {
                    let Some(selected) = carrier.and_then(|carrier| {
                        self.program.data_constructor_field(
                            carrier,
                            value.type_symbol,
                            field.field_symbol,
                        )
                    }) else {
                        self.expression(field.value, contexts, constraints);
                        continue;
                    };
                    let mut path = Vec::with_capacity(2);
                    if let Some(variant) =
                        facts::payload_variant_for_field(self.program, selected.symbol)
                    {
                        path.push(facts::PlaceSegment::Case { variant });
                    }
                    path.push(facts::PlaceSegment::Field {
                        symbol: selected.symbol,
                    });
                    let child_type = self.constructed_child_type(expression, destination, &path);
                    let evaluated =
                        self.evaluated_operand_as(field.value, child_type, contexts, constraints);
                    self.append_owned_value_observations(
                        expression,
                        &path,
                        child_type,
                        &evaluated,
                        contexts,
                        constraints,
                    );
                    self.append_operand_child(
                        expression,
                        &path,
                        child_type,
                        evaluated.capture,
                        &mut children,
                    );
                }
                if let Some(variant) = value.case_symbol.filter(|variant| variant.is_valid())
                    && self.program.symbols.get(variant).kind == symbols::SymbolKind::Variant
                    && self.program.symbols.get(variant).parent == value.type_symbol
                {
                    self.append_value_observation(
                        expression,
                        &[],
                        facts::FactPayload::CaseValue {
                            variant,
                            value: true,
                        },
                        contexts,
                        constraints,
                    );
                }
                *completion = checked_trees::FlowOperandCompletion::Constructed(children);
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start, contexts, constraints);
                self.expression(range.end, contexts, constraints);
            }
            ExpressionNode::Indexed(indexed) => {
                let collection_operand = self
                    .evaluated_place(indexed.collection, contexts, constraints)
                    .capture;
                *backing = self.context.control.operands.insert(collection_operand);
                let selector_writes_start = self.operand_writes.len();
                if indexed.index.is_valid()
                    && let ExpressionNode::Range(range) =
                        self.program.expression_table.expression(indexed.index)
                {
                    *retention = Some(collection_operand.retention);
                    let start_operand = self
                        .evaluated_operand(range.start, contexts, constraints)
                        .capture;
                    let end_operand = self
                        .evaluated_operand(range.end, contexts, constraints)
                        .capture;
                    *evaluated_inputs = self
                        .context
                        .control
                        .operands
                        .insert_many([start_operand, end_operand]);
                    *referents = Some(self.capture_indexed_referents(
                        expression,
                        &collection_operand,
                        &start_operand,
                        Some(&end_operand),
                        possible_storage,
                        address_projection,
                    ));
                    self.append_result_facts(indexed.index, contexts, constraints);
                    self.record_operator_invocation(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::Expression,
                        &[collection_operand, start_operand, end_operand],
                        *constraints,
                    );
                } else {
                    let index_operand = self
                        .evaluated_operand(indexed.index, contexts, constraints)
                        .capture;
                    *evaluated_inputs = self.context.control.operands.insert_many([index_operand]);
                    *retention = Some(self.capture_indexed_retention(
                        expression,
                        &collection_operand,
                        &index_operand,
                        selector_writes_start,
                    ));
                    *referents = Some(self.capture_indexed_referents(
                        expression,
                        &collection_operand,
                        &index_operand,
                        None,
                        possible_storage,
                        address_projection,
                    ));
                    self.record_operator_invocation(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::Expression,
                        &[collection_operand, index_operand],
                        *constraints,
                    );
                }
                if crate::values::operator_is_builtin(self.context.operators, expression) {
                    *case_access = self.record_case_access_places(
                        expression,
                        *constraints,
                        referents.unwrap_or_default(),
                    );
                    self.retain_value_observation(
                        expression,
                        *backing,
                        referents.unwrap_or_default(),
                        *contexts,
                        *constraints,
                    );
                }
            }
            ExpressionNode::Atomic(_) => {
                let own_writes =
                    validation::atomic_once_carrier(self.program, expression).map(|_| {
                        super::mutation::atomic_once_write_effects(
                            self.program,
                            self.machine.symbol,
                            self.state.symbol,
                            self.statement_index,
                            expression,
                            self.context.call_frames,
                        )
                    });
                if let Some(inputs) = validation::atomic_value_operands(self.program, expression) {
                    let load = validation::atomic_load_carrier(self.program, expression).is_some();
                    let resident =
                        validation::atomic_resident_type_reference(self.program, destination);
                    let mut captured = Vec::with_capacity(inputs.len());
                    for input in inputs {
                        let reference = if load {
                            self.context.expression_result_type_at(
                                self.program,
                                self.machine,
                                self.state,
                                input,
                            )
                        } else {
                            resident
                        }
                        .unwrap_or_default();
                        captured.push(
                            self.evaluated_operand_as(input, reference, contexts, constraints)
                                .capture,
                        );
                    }
                    let operands = self.context.control.operands.insert_many(captured);
                    self.context.control.atomic_executions.append(
                        checked_trees::flow::FlowAtomicExecutionFact {
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                            expression,
                            operands,
                        },
                    );
                }
                if let Some(writes) = own_writes {
                    self.record_operand_writes(match &writes {
                        Some((bindings, observations)) => OperandMutation {
                            bindings: Some(bindings.clone()),
                            observations: Some(observations.clone()),
                        },
                        None => OperandMutation {
                            bindings: None,
                            observations: None,
                        },
                    });
                    if let Some((_, writes)) = writes {
                        self.filter_captured_sources(&writes, contexts, constraints);
                    } else {
                        *contexts = HandleSpan::empty();
                        *constraints = HandleSpan::empty();
                    }
                }
            }
            ExpressionNode::Cast(cast) => {
                let evaluated = if cast.form.is_recast() {
                    self.evaluated_place(cast.value, contexts, constraints)
                } else {
                    self.evaluated_operand(cast.value, contexts, constraints)
                };
                *retention = Some(if cast.form.is_recast() {
                    self.reinterpreted_view_retention(
                        &evaluated.capture,
                        evaluated.capture.retention,
                    )
                } else {
                    evaluated.capture.retention
                });
                *evaluated_inputs = self
                    .context
                    .control
                    .operands
                    .insert_many([evaluated.capture]);
                if validation::value_preserving_cast_operand(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                ) == Some(cast.value)
                {
                    *boolean_completion = evaluated.boolean_completion;
                    return evaluated.boolean;
                }
            }
            ExpressionNode::Borrow(borrow) => {
                let previous_purpose = self.read_purpose;
                let previous_slot = self.reference_slot_target;
                if validation::borrow_selects_reference_storage(
                    self.program,
                    borrow.target,
                    destination,
                    &[],
                ) {
                    self.reference_slot_target = borrow.target;
                }
                self.read_purpose = ReadPurpose::Address;
                let evaluated = self.evaluated_place(borrow.target, contexts, constraints);
                self.read_purpose = previous_purpose;
                self.reference_slot_target = previous_slot;
                *backing = self.context.control.operands.insert(evaluated.capture);
                *referents = Some(evaluated.capture.referents);
                *retention = Some(self.formed_view_retention(&evaluated.capture));
            }
            ExpressionNode::Unary(unary) => {
                let operand = self.evaluated_operand(unary.operand, contexts, constraints);
                *evaluated_inputs = self.context.control.operands.insert_many([operand.capture]);
                if unary.operator == UnaryOperator::LogicalNot
                    && crate::values::operator_is_builtin(self.context.operators, expression)
                {
                    *boolean_completion =
                        self.negated_boolean_completion(operand.boolean_completion);
                    return operand.boolean.map(|value| !value);
                }
            }
            ExpressionNode::String(_) => {
                let image = self.append_retention(&[]);
                self.context
                    .control
                    .operand_retentions
                    .get_mut(image)
                    .immutable_image = true;
                *retention = Some(image);
                *formed_type = destination;
            }
            ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        None
    }

    fn record_case_access(
        &mut self,
        expression: ExpressionHandle,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> Handle<checked_trees::FlowCaseAccessFact> {
        let Some(place) = self.completed_read_place(expression) else {
            return Handle::invalid();
        };
        let places = self.append_referent_places(&[place]);
        self.record_case_access_places(expression, constraints, places)
    }

    /*
        Where `expression`'s place leaves the descriptor it reads through. A
        Case segment inside that descriptor's own home was the obligation of
        the read that loaded the descriptor; a read through the loaded
        descriptor proves only the cases on its pointee path, and an external
        view with no placeable pointee owes none.
    */
    fn pointee_segments_start(&mut self, expression: ExpressionHandle) -> usize {
        let mut current = expression;
        loop {
            let receiver = match self.program.expression_table.expression(current) {
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Indexed(indexed) => indexed.collection,
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => return 0,
            };
            let crosses = self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, receiver)
                .is_some_and(|reference| self.is_view_type(reference));
            if crosses {
                return place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    receiver,
                )
                .map_or(0, |home| home.segments.len());
            }
            current = receiver;
        }
    }

    fn record_case_access_places(
        &mut self,
        expression: ExpressionHandle,
        constraints: HandleSpan<FlowConstraintRef>,
        places: HandleSpan<checked_trees::FlowOperandReferent>,
    ) -> Handle<checked_trees::FlowCaseAccessFact> {
        let pointee_start = self.pointee_segments_start(expression);
        let source_has_case = place_at(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )
        .is_some_and(|place| {
            place
                .segments
                .iter()
                .skip(pointee_start)
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        });
        let observed_has_case = self.captured_referent_places(places).iter().any(|place| {
            place
                .segments
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        });
        let formed_reference = self
            .context
            .control
            .operand_referents
            .span_or_empty(places)
            .iter()
            .any(|place| place.formation.is_valid());
        let descriptor_read = !places.is_empty()
            && expression != self.reference_slot_target
            && self
                .context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .is_some_and(|reference| self.is_view_type(reference));
        if source_has_case || observed_has_case || formed_reference || descriptor_read {
            return self
                .context
                .control
                .case_accesses
                .append(checked_trees::FlowCaseAccessFact {
                    machine_symbol: self.machine.symbol,
                    state_symbol: self.state.symbol,
                    expression,
                    evaluation: self.evaluation,
                    statement_index: self.statement_index,
                    constraints,
                    places,
                });
        }
        Handle::invalid()
    }

    fn live_boolean(
        &mut self,
        expression: ExpressionHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<bool> {
        let place = self.completed_read_place(expression)?;
        match self
            .context
            .scalar_value_at_place(self.program, self.semantic, contexts, &place)?
        {
            facts::ScalarValue::Boolean(value) => Some(value),
            _ => None,
        }
    }

    fn changed_operand_sources(
        &mut self,
        operands: &[(ExpressionHandle, usize)],
    ) -> Vec<CanonicalPlace> {
        let mut changed = Vec::new();
        for (expression, first_write) in operands {
            if *first_write == self.operand_writes.len() {
                continue;
            }
            if matches!(
                self.program.expression_table.expression(*expression),
                ExpressionNode::Call(_)
            ) {
                continue;
            }
            let occurrences = self
                .context
                .expression_occurrences_at(self.program, *expression);
            for occurrence in occurrences.iter().copied() {
                let Some(place) = self.context.canonical_place_at(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    occurrence,
                ) else {
                    continue;
                };
                for mutation in &self.operand_writes[*first_write..] {
                    let writes = &mutation.observations;
                    let Some(writes) = writes else {
                        if !changed.contains(&place) {
                            changed.push(place.clone());
                        }
                        continue;
                    };
                    for write in writes {
                        if place.root != write.root
                            || !canonical_place_segments_may_overlap(
                                self.program,
                                &place.segments,
                                &write.segments,
                            )
                        {
                            continue;
                        }
                        let exact_prefix =
                            place.segments.len() < write.segments.len()
                                && place.segments.iter().zip(&write.segments).all(
                                    |(left, right)| {
                                        !matches!(left, facts::PlaceSegment::Index { .. })
                                            && super::canonical_place_segments_equal(
                                                self.program,
                                                *left,
                                                *right,
                                            )
                                    },
                                );
                        let affected = if exact_prefix {
                            CanonicalPlace {
                                root: place.root,
                                segments: write.segments.clone(),
                            }
                        } else {
                            place.clone()
                        };
                        crate::shared::collections::push_unique(&mut changed, affected);
                    }
                }
            }
        }
        changed
    }

    fn filter_captured_sources(
        &mut self,
        changed: &[CanonicalPlace],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if changed.is_empty() {
            return;
        }
        let mut changed = changed.to_vec();
        super::entry_origins::extend_invocation_observations(
            self.program,
            self.context,
            self.machine,
            self.state,
            self.statement_index,
            &mut changed,
        );
        *contexts = filter_contexts_after_place_mutations(
            self.program,
            self.semantic,
            self.domains,
            &mut self.context.contexts.semantic_context_refs,
            &mut self.context.invalidations.segments,
            &mut self.context.invalidations.events,
            *contexts,
            &changed,
            FlowInvalidationSource::Statement {
                statement_index: self.statement_index,
            },
        );
        *constraints = project_constraint_refs_to_active_contexts(
            &mut self.context.contexts.constraint_refs,
            *constraints,
            *contexts,
            &self.context.contexts.semantic_context_refs,
        );
    }

    fn meet(
        &mut self,
        skipped: HandleSpan<FlowSemanticContextRef>,
        skipped_constraints: HandleSpan<FlowConstraintRef>,
        evaluated: &mut HandleSpan<FlowSemanticContextRef>,
        evaluated_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let mut memberships = self.live_memberships(skipped);
        let incoming = self.live_memberships(*evaluated);
        live_memberships::retain_common(&mut memberships, &incoming);
        let retained = reference_spans::intersect_semantic_contexts_in_order(
            self.context
                .contexts
                .semantic_context_refs
                .span_or_empty(skipped),
            self.context
                .contexts
                .semantic_context_refs
                .span_or_empty(*evaluated),
        );
        *evaluated = HandleSpan::empty();
        for reference in retained {
            reference_spans::append_flow_reference(
                &mut self.context.contexts.semantic_context_refs,
                evaluated,
                reference,
            );
        }
        let retained = reference_spans::meet_constraint_references_in_order(
            self.context
                .contexts
                .constraint_refs
                .span_or_empty(skipped_constraints),
            self.context
                .contexts
                .constraint_refs
                .span_or_empty(*evaluated_constraints),
        );
        *evaluated_constraints = HandleSpan::empty();
        for reference in retained {
            reference_spans::append_flow_reference(
                &mut self.context.contexts.constraint_refs,
                evaluated_constraints,
                reference,
            );
        }
        *evaluated_constraints = project_constraint_refs_to_active_contexts(
            &mut self.context.contexts.constraint_refs,
            *evaluated_constraints,
            *evaluated,
            &self.context.contexts.semantic_context_refs,
        );
        self.publish_live_memberships(memberships, evaluated, evaluated_constraints);
    }
}

/*
    The variant every ordinary return of the callee entry constructs directly.
    A parameter, a call, or disagreeing arms do not invent a tag.
*/
fn returned_constructor_case(
    program: &dyn ::validation::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<symbols::SymbolHandle> {
    let (callee, state) = validation::find_state(program, call.target_symbol)?;
    if !callee.body_is_present || program.machine_states(callee).first()?.symbol != state.symbol {
        return None;
    }
    let values = typed_trees::statement::direct_state_return_values(
        &program.statement_table,
        program.statement_table.statements(state.statement_nodes),
    )?;
    let mut agreed = None;
    for value in values {
        let variant = constructor_case(program, value)?;
        if agreed.is_some_and(|existing| existing != variant) {
            return None;
        }
        agreed = Some(variant);
    }
    agreed
}

fn constructor_case(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let ExpressionNode::StructLiteral(value) = program.expression_table.expression(expression)
    else {
        return None;
    };
    let variant = value.case_symbol.filter(|variant| variant.is_valid())?;
    let symbol = program.symbols.get(variant);
    if symbol.kind == symbols::SymbolKind::Variant && symbol.parent == value.type_symbol {
        Some(variant)
    } else {
        None
    }
}
