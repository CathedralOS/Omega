//! Complete selected payload copies before target operands. Saved receiver
//! observations authenticate the source address; source expressions identify
//! occurrences but are never evaluated again. Reference leaves save descriptors
//! at this boundary and retain ordinary live-memory semantics after it.

use super::{Execution, ReadPurpose};
use arena::HandleSpan;
use checked_trees::{
    FlowConstraintKind, FlowConstraintRef, FlowOperandCompletion, FlowOperandFact,
    FlowSemanticContextRef,
};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{TableTransition, TransitionTargetHandle, TransitionTargetNode};

impl Execution<'_, '_, '_> {
    /// Read the value named by this occurrence. Address formation deliberately
    /// keeps the original source place; saved reference descriptors resolve to
    /// their actual referent without freezing that referent's current contents.
    pub(super) fn completed_read_place(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::CanonicalPlace> {
        if self.read_purpose == ReadPurpose::Address {
            return validation::canonical_place_from_expression_in_state(
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
        let place = validation::canonical_place_from_expression_in_state(
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
        // Continuation operands do not name this arm's extracted payloads.
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
        let expressions = match self.program.statement_table.transition_target(target) {
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
            &expressions,
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
                    self.program
                        .expression_table
                        .expressions_structurally_equal(*previous, payload)
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
                self.program
                    .expression_table
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
            // The guard's loan must still be active at the actual copy boundary.
            // A later conjunct cannot donate a retired descriptor or new address.
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
            if !retained_loan || !self.copy_source_coordinates_agree(statements, &source, &entry) {
                continue;
            }
            let symbol = validation::effective_member_symbol(self.program, member.receiver, member);
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
            // This is the actual source read. Its live tag must independently
            // hold at selection, before any target operand can change storage.
            let source_places = self.append_captured_references(&payload_places);
            let source_access =
                self.record_case_access_places(payload, *constraints, source_places);
            self.publish_copy_contents(payload, contexts, constraints);
            let mut captured = entry;
            captured.case_access = source_access;
            captured.constraints = *constraints;
            captured.retention = self.capture_value_retention(&captured);
            let source_handle = self.context.control.operands.append(captured);
            let references = self.capture_owned_reference_leaves(source_handle, &payload_places);
            let copy = self
                .context
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
        let Some(mut source) = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            payload,
        ) else {
            return;
        };
        let active = super::super::active_context_handles(self.context, *contexts);
        super::super::narrow_pinned_index_segments(
            self.program,
            self.semantic,
            self.state.symbol,
            self.statement_index,
            &active,
            &mut source.segments,
        );
        let source =
            crate::semantic::places::append_place(self.semantic, source.root, &source.segments);
        let destination = crate::semantic::places::append_place(
            self.semantic,
            facts::PlaceRoot::Expression(payload),
            &[],
        );
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
        let Some(coordinate) = validation::canonical_place_from_expression_in_state(
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
                validation::canonical_place_from_expression_in_state(
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

    /// A by-value read projects one completed copy. Address formation deliberately
    /// keeps the ordinary source lane. Crossing a copied reference ends this
    /// owned-value projection and resumes live-memory observation.
    pub(super) fn dispatch_copy_value(
        &mut self,
        expression: ExpressionHandle,
        constraints: HandleSpan<FlowConstraintRef>,
    ) -> Option<FlowOperandFact> {
        if self.read_purpose != ReadPurpose::Value {
            return None;
        }
        let place = validation::canonical_place_from_expression_in_state(
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
            let prefix = validation::canonical_place_from_expression_in_state(
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
                        super::super::canonical_place_segments_equal(self.program, *left, *right)
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
                // A matched copy remains the value owner even when its saved
                // descriptor has no exact origin. Empty inventory is unknown;
                // it must not select the ordinary source descriptor as fallback.
                projected.referents = HandleSpan::empty();
                projected.retention = arena::Handle::invalid();
                if let Some(leaf) = leaves.next()
                    && leaves.next().is_none()
                {
                    projected.referents = leaf.referents;
                    projected.retention = leaf.retention;
                }
                // The descriptor was saved, not its referee's contents. Keep
                // the current constraints after prior target operand effects.
            } else {
                // This address belongs to the immutable completed value. Borrow
                // targets never enter this lane; they retain source storage.
                projected.referents = self.append_referent_places(&[validation::CanonicalPlace {
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
