//! Field observation consumes either established storage or a complete evaluated
//! receiver. Both keep the same declared projection; a fresh receiver is an
//! ordinary structural value dependency, never just its selected initializer.
//! A literal element of a fresh receiver's scalar array is observed the same
//! way: the receiver has no storage for an indexed read to name, so its
//! completed value supplies the element. Established storage keeps its
//! indexed-read selection, which also serves runtime selectors.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    ExpressionNode,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

/// A typed scalar observation keeps the complete source carrier and final
/// qualified scalar type beside each explicit readable descriptor crossing.
/// It is preparation data, not a grant to read or copy the source value.
pub(super) struct ScalarObservation {
    pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
    pub(super) primitive_type: typed_trees::types::PrimitiveType,
    pub(super) domain: numerics::arithmetic::ArithmeticDomain,
    result_type: typed_trees::types::TypeReferenceHandle,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
}

impl Builder<'_, '_> {
    fn scalar_observation_type(
        &self,
        source_type: typed_trees::types::TypeReferenceHandle,
    ) -> Option<ScalarObservation> {
        use typed_trees::types::TypeReferenceNode;
        let mut reference = source_type;
        let mut result_type = source_type;
        let mut visited = Vec::new();
        let mut path = Vec::new();
        loop {
            if !self
                .program
                .type_reference_table
                .contains_type_reference(reference)
                || visited.contains(&reference)
            {
                return None;
            }
            visited.push(reference);
            // A stored view is already a descriptor value. Its elements do
            // not become a scalar by interpreting its spelling as a pointer.
            if validation::stored_view_shell(self.program, reference, &[]).is_some() {
                return None;
            }
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Reference {
                    referee, access, ..
                } if access.is_readable() => {
                    path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                    result_type = *referee;
                    reference = *referee;
                }
                TypeReferenceNode::Reference { .. } => return None,
                _ => {
                    let primitive_type = self.program.primitive_type_reference(reference)?;
                    return Some(ScalarObservation {
                        type_reference: source_type,
                        result_type,
                        primitive_type,
                        domain: self
                            .program
                            .arithmetic_domain_for_type_reference(result_type),
                        path,
                    });
                }
            }
        }
    }

    fn publish_scalar_observation(
        &mut self,
        expression: ExpressionHandle,
        receiver_value: checked_trees::CheckedStructuralValueHandle,
        observation: ScalarObservation,
    ) -> CheckedScalarComputationHandle {
        self.insert(
            observation.primitive_type,
            CheckedScalarComputationKind::StructuralField {
                atomic_ordering: None,
                result_type: observation.result_type,
                source_expression: expression,
                receiver_expression: expression,
                source_type: observation.type_reference,
                receiver_value,
                subject: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                    },
                    path: observation.path,
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        observation.type_reference,
                        &[],
                        &[],
                    ),
                    access: CheckedStructuralAccess::SharedBorrow,
                },
                field: symbols::SymbolHandle::invalid(),
                field_identity: Default::default(),
            },
        )
    }
    /// Read the scalar endpoint of an ordinary completed value projection.
    /// The projection owns selection from the saved descriptor, including any
    /// subsequent live reference crossings. It never re-reads the old carrier.
    pub(super) fn completed_scalar_observation(
        &mut self,
        expression: ExpressionHandle,
        expected: typed_trees::types::PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let reference = validation::expression_type_reference_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let observation = self.scalar_observation_type(reference)?;
        if observation.primitive_type != expected {
            return None;
        }
        // This is the actual selected-copy receiver. Its captured descriptor
        // must complete before a scalar consumer follows its referent; a live
        // read of the old source slot is never a substitute for this value.
        let receiver_value = self.retain_structural_argument(expression, reference)?;
        Some(self.publish_scalar_observation(expression, receiver_value, observation))
    }

    /// A scalar read finishes its fresh reference receiver before a later
    /// operand can write the backing. The captured scalar is independent; the
    /// descriptor is not an owned referent or a receiver kept until scope exit.
    /// Nominal temporary owners retain their separately checked normal death.
    pub(super) fn observation_completion_actions(
        &mut self,
        kind: &CheckedScalarComputationKind,
    ) -> arena::HandleSpan<checked_trees::CheckedNormalCleanupAction> {
        let CheckedScalarComputationKind::StructuralField { receiver_value, .. } = kind else {
            return arena::HandleSpan::empty();
        };
        if !receiver_value.is_valid() {
            return arena::HandleSpan::empty();
        }
        let receiver = self.structural_values.nodes.get(*receiver_value);
        if !validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            receiver.expression,
        )
        .is_some_and(|place| {
            place.root == facts::PlaceRoot::Expression(receiver.expression)
                && place.segments.is_empty()
        }) {
            return arena::HandleSpan::empty();
        }
        if !validation::reference_shell(self.program, receiver.type_reference)
            .is_some_and(|shell| shell.access.is_readable())
        {
            return arena::HandleSpan::empty();
        }
        let source = CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
            expression: receiver.expression,
        };
        let Some(actions) = crate::execution::terminal_unit::cleanup::descriptor_retirement(
            self.program,
            source,
            receiver.type_reference,
        ) else {
            return arena::HandleSpan::empty();
        };
        self.plans.temporary_cleanup_actions.insert_many(actions)
    }

    /// A computed reference first produces its permission carrier; a scalar
    /// receiving position then observes that carrier's referent. This is the
    /// same explicit storage read as a field projection, not a scalar ABI for
    /// the producer and not an equation to its possible ingress origins.
    pub(super) fn primitive_reference_observation(
        &mut self,
        expression: ExpressionHandle,
        expected: typed_trees::types::PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let observation = self.primitive_reference_observation_type(expression)?;
        if observation.primitive_type != expected {
            return None;
        }
        let reference = observation.type_reference;
        let receiver_value = self.structural_value(expression, reference)?;
        let statement = u32::try_from(self.statement_index).ok()?;
        self.retain_structural_root(statement, expression, reference, receiver_value);
        Some(self.publish_scalar_observation(expression, receiver_value, observation))
    }

    /// Receiving positions and arithmetic operands ask the same type question.
    /// The reference's declared referee supplies its scalar carrier and policy;
    /// neither a surrounding operator nor the return ABI may invent them.
    pub(super) fn primitive_reference_observation_type(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScalarObservation> {
        if validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )
        .is_some_and(|place| place.root.as_symbol().is_some())
        {
            // Established storage already has the ordinary named/projected
            // observation route. It must not be copied into another home.
            return None;
        }
        let reference = validation::expression_type_reference_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let observation = self.scalar_observation_type(reference)?;
        // This entrance is specifically a computed reference read. Direct
        // primitive captured values use the same typed observation above.
        (!observation.path.is_empty()).then_some(observation)
    }

    pub(super) fn structural_scalar_field(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::StructuralScalarField> {
        let element = match self.program.expression_table.expression(expression) {
            ExpressionNode::Member(_) => false,
            ExpressionNode::Indexed(_) => true,
            _ => return None,
        };
        if element {
            return self
                .structural_receiver_scalar(expression)
                .filter(|field| field.receiver_value.is_valid());
        }
        if !self.declaration_parameters.is_empty() {
            return validation::structural_scalar_field_in_parameters(
                self.program,
                self.declaration_parameters,
                self.declaration_bindings,
                expression,
            );
        }
        self.captured_case_scalar_field(expression)
            .or_else(|| self.structural_receiver_scalar(expression))
    }

    /// Pattern-bound copies belong to the selected subject's already captured
    /// scalar payload namespace. Rejoin its exact completed descriptor rather
    /// than retaining another read of a dynamic source projection. Stage 05's
    /// ordinary case payload bindings supply these leaves before all target
    /// operands; arithmetic and calls still use their shared computation graph.
    fn captured_case_scalar_field(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::StructuralScalarField> {
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_in_machine(machine, self.state)?;
        let observations = crate::flow::CapturedSubjectFacts {
            semantic: self.semantic,
            flow: self.flow,
            borrow: self.borrow,
        };
        let transition = self
            .program
            .statement_table
            .statement_at(state, self.statement_index)?
            .as_transition()?;
        let (_, copy) = crate::flow::copied_payload_at(
            self.program,
            self.flow,
            self.machine,
            self.state,
            self.statement_index,
            checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target),
            expression,
        )?;
        if self
            .program
            .primitive_type_reference(copy.type_reference)
            .is_none()
            || validation::reference_shell(self.program, copy.type_reference).is_some()
        {
            return None;
        }
        let flow_state = self.flow.control.unique_state(self.machine, self.state)?;
        let statement = self
            .flow
            .state_statement(flow_state, self.statement_index)?;
        let mut selected = None;
        for observation in self
            .flow
            .control
            .value_observations
            .span_or_empty(statement.value_observations)
        {
            let Some(subject) = crate::flow::captured_case_subject(
                self.program,
                observations,
                machine,
                state,
                self.statement_index,
                observation.expression,
            ) else {
                continue;
            };
            if subject.receiver != copy.backing {
                continue;
            }
            let captured = self.flow.control.operands.get(subject.source);
            let source_place = validation::canonical_place_from_expression_in_state(
                self.program,
                captured.state_symbol,
                captured.statement_index,
                captured.expression,
            )?;
            let mut receiver = expression;
            loop {
                receiver = match self.program.expression_table.expression(receiver) {
                    ExpressionNode::Member(member) => member.receiver,
                    ExpressionNode::Indexed(indexed) => indexed.collection,
                    _ => break,
                };
                let Some(place) = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    receiver,
                ) else {
                    continue;
                };
                if !crate::flow::canonical_places_agree(self.program, &place, &source_place) {
                    continue;
                }
                let source = validation::structural_source(
                    self.program,
                    self.machine,
                    self.state,
                    u32::try_from(self.statement_index).ok()?,
                    expression,
                    subject.observed,
                )?;
                let field = validation::structural_scalar_field_of_evaluated_receiver(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                    receiver,
                    source,
                )?;
                if selected.as_ref().is_some_and(|previous| previous != &field) {
                    return None;
                }
                selected = Some(field);
                break;
            }
        }
        selected
    }

    fn structural_receiver_scalar(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::StructuralScalarField> {
        validation::structural_scalar_field(
            self.program,
            self.machine,
            self.state,
            expression,
            &[],
            |receiver| {
                validation::structural_source(
                    self.program,
                    self.machine,
                    self.state,
                    u32::try_from(self.statement_index).ok()?,
                    expression,
                    receiver,
                )
            },
        )
    }

    /// Observe the common formal-relative coordinate from an immutable entry
    /// snapshot. Empty paths use the scalar telescope; projected paths use the
    /// same declared structural field operation as ordinary source reads.
    pub(super) fn scalar_coordinate(
        &mut self,
        expression: ExpressionHandle,
        coordinate: &validation::CanonicalPlace,
        expected_type: typed_trees::types::PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let facts::PlaceRoot::Symbol(symbol) = coordinate.root else {
            return None;
        };
        if coordinate.segments.is_empty() {
            let position = self
                .parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)?;
            if self.parameter_types.get(position) != Some(&expected_type) {
                return None;
            }
            return Some(self.insert(
                expected_type,
                CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::Parameter {
                        position,
                        primitive_type: expected_type,
                    },
                ),
            ));
        }
        let field = validation::structural_scalar_field_at_coordinate(
            self.program,
            self.authored_parameters,
            symbol,
            &coordinate.segments,
        )?;
        if field.primitive_type != expected_type {
            return None;
        }
        self.structural_field(expression, field)
    }

    pub(super) fn structural_field(
        &mut self,
        expression: ExpressionHandle,
        field: validation::StructuralScalarField,
    ) -> Option<CheckedScalarComputationHandle> {
        let receiver_value = if let Some(captured) = self.captured_dispatch_value(expression)
            && captured.type_reference == field.type_reference
            && field.root == facts::PlaceRoot::Expression(captured.expression)
        {
            captured.root
        } else if field.receiver_value.is_valid() {
            let value = self.structural_value(field.receiver_value, field.type_reference)?;
            self.retain_structural_value_root(value, field.type_reference)?;
            value
        } else {
            checked_trees::CheckedStructuralValueHandle::invalid()
        };
        let source = match field.root {
            facts::PlaceRoot::Symbol(symbol) => self.structural_source(symbol),
            facts::PlaceRoot::Expression(expression) => {
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression }
            }
            _ => return None,
        };
        Some(
            self.insert(
                field.primitive_type,
                CheckedScalarComputationKind::StructuralField {
                    atomic_ordering: None,
                    result_type: field.result_type_reference,
                    source_expression: expression,
                    receiver_expression: match self.program.expression_table.expression(expression)
                    {
                        ExpressionNode::Member(member) => self
                            .program
                            .saved_argument_source(member.receiver)
                            .unwrap_or(member.receiver),
                        ExpressionNode::Indexed(indexed) => indexed.collection,
                        _ => ExpressionHandle::invalid(),
                    },
                    source_type: field.type_reference,
                    receiver_value,
                    subject: CheckedUnitStructuralArgumentPlan {
                        projection_evaluations: Default::default(),
                        source,
                        path: field.path,
                        type_identity: validation::structural_type_identity(
                            self.program,
                            field.carrier_type_reference,
                            &[],
                            &[],
                        ),
                        access: CheckedStructuralAccess::SharedBorrow,
                    },
                    field: field.field,
                    field_identity: field.field_identity,
                },
            ),
        )
    }
}
