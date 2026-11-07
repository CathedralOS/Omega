//! Field observation consumes either established storage or a complete evaluated
//! receiver. Both keep the same declared projection; a fresh receiver is an
//! ordinary structural value dependency, never just its selected initializer.
//! A literal element of a fresh receiver's scalar array is observed the same
//! way: the receiver has no storage for an indexed read to name, so its
//! completed value supplies the element. Established storage keeps its
//! indexed-read selection, which also serves runtime selectors.
use super::{
    Builder, CheckedScalarComputationHandle as ComputationHandle,
    CheckedScalarComputationKind as ComputationKind, ExpressionHandle, ExpressionNode,
};
use crate::published::places::place_at;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
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
    ) -> ComputationHandle {
        self.insert(
            observation.primitive_type,
            ComputationKind::StructuralField {
                atomic_ordering: None,
                result_type: observation.result_type,
                source_expression: expression,
                receiver_expression: expression,
                source_type: observation.type_reference,
                receiver_value,
                subject: CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: SourcePlan::StructuralExpression {
                        expression,
                        logical_root_type_identity: None,
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

    /// Read one scalar leaf `row[indices]` a runtime subvalue row contributes
    /// to a scalar array construction. The row's canonical place supplies the
    /// receiver; the leaf is an ordinary storage observation at its declared
    /// carrier, never a move or re-evaluation of the row's owner.
    pub(super) fn projected_element_scalar(
        &mut self,
        row: ExpressionHandle,
        indices: &[u32],
        leaf_type: typed_trees::types::TypeReferenceHandle,
        primitive_type: typed_trees::types::PrimitiveType,
    ) -> Option<ComputationHandle> {
        let coordinate = self.canonical_place(row)?;
        let facts::PlaceRoot::Symbol(symbol) = coordinate.root else {
            return None;
        };
        let mut segments = coordinate.segments.clone();
        for index in indices {
            segments.push(facts::PlaceSegment::FixedIndex {
                index: usize::try_from(*index).ok()?,
            });
        }
        let field = validation::structural_scalar_field_at_element_coordinate(
            self.program,
            self.state,
            symbol,
            &segments,
        )?;
        if field.primitive_type != primitive_type
            || self
                .program
                .normalized_type_identity(field.result_type_reference)
                != self.program.normalized_type_identity(leaf_type)
        {
            return None;
        }
        self.structural_field(row, field)
    }

    /// A projected element leaf records at the same `ArrayElement` coordinate
    /// an authored leaf would, so the result consumer reads one vocabulary.
    pub(super) fn record_projected_element_root(
        &mut self,
        statement_ordinal: u32,
        role: checked_trees::CheckedScalarExpressionRole,
        row: ExpressionHandle,
        indices: &[u32],
        leaf_type: typed_trees::types::TypeReferenceHandle,
        primitive_type: typed_trees::types::PrimitiveType,
    ) {
        if self
            .plans
            .root_at(self.state, statement_ordinal, role)
            .is_some()
        {
            return;
        }
        if let Some(root) = self.projected_element_scalar(row, indices, leaf_type, primitive_type) {
            self.plans.nodes.get_mut(root).authored_root = row;
            self.plans
                .roots
                .append(checked_trees::CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal,
                    role,
                    root,
                });
        }
    }

    /// Read the scalar endpoint of an ordinary completed value projection.
    /// The projection owns selection from the saved descriptor, including any
    /// subsequent live reference crossings. It never re-reads the old carrier.
    pub(super) fn completed_scalar_observation(
        &mut self,
        expression: ExpressionHandle,
        expected: typed_trees::types::PrimitiveType,
    ) -> Option<ComputationHandle> {
        let reference = self.expression_type(expression)?;
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

    /// A scalar field or tag read finishes its fresh reference receiver before a later
    /// operand can write the backing. The captured scalar is independent; the
    /// descriptor is not an owned referent or a receiver kept until scope exit.
    /// Nominal temporary owners retain their separately checked normal death.
    pub(super) fn observation_completion_actions(
        &mut self,
        kind: &ComputationKind,
    ) -> arena::HandleSpan<checked_trees::CheckedNormalCleanupAction> {
        let receiver_value = kind.observed_receiver();
        if !receiver_value.is_valid() {
            return arena::HandleSpan::empty();
        }
        let receiver = self.structural_values.nodes.get(receiver_value);
        if !self
            .canonical_place(receiver.expression)
            .is_some_and(|place| {
                place.root == facts::PlaceRoot::Expression(receiver.expression)
                    && place.segments.is_empty()
            })
        {
            return arena::HandleSpan::empty();
        }
        if !validation::reference_shell(self.program, receiver.type_reference)
            .is_some_and(|shell| shell.access.is_readable())
        {
            return arena::HandleSpan::empty();
        }
        let source = SourcePlan::StructuralExpression {
            expression: receiver.expression,
            logical_root_type_identity: None,
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
    ) -> Option<ComputationHandle> {
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
        if self
            .canonical_place(expression)
            .is_some_and(|place| place.root.as_symbol().is_some())
        {
            // Established storage already has the ordinary named/projected
            // observation route. It must not be copied into another home.
            return None;
        }
        let reference = self.expression_type(expression)?;
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
    /// than retaining another read of a dynamic source projection. Psi 06's
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
            let source_place = place_at(
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
                let Some(place) = self.canonical_place(receiver) else {
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

    pub(super) fn structural_field(
        &mut self,
        expression: ExpressionHandle,
        field: validation::StructuralScalarField,
    ) -> Option<ComputationHandle> {
        // A saved operand is an expression or local home, not a fresh value to
        // evaluate. Specialization reads a value-parameter descriptor from
        // that operand after erasure has removed the carrier's runtime place.
        // The closed integer operand of the application is the field's
        // established value, so the observation does not borrow storage the
        // carrier does not have.
        if let Some(literal) = closed_descriptor_value(self.program, &field) {
            return Some(self.insert(
                field.primitive_type,
                ComputationKind::Value(checked_trees::CheckedScalarExpression::IntegerLiteral {
                    literal,
                }),
            ));
        }
        // The parameter place keeps the descriptor as a runtime value parameter
        // of the specialized machine. That parameter is the saved integer; the
        // erased carrier still has no storage to borrow.
        if let Some(symbol) = descriptor_parameter_symbol(self.program, &field)
            && let Some(position) = self
                .parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)
            && self.parameter_types.get(position) == Some(&field.primitive_type)
        {
            return Some(self.insert(
                field.primitive_type,
                ComputationKind::Value(checked_trees::CheckedScalarExpression::Parameter {
                    position,
                    primitive_type: field.primitive_type,
                }),
            ));
        }
        let receiver_value = if let Some(captured) = self.captured_dispatch_value(expression)
            && captured.type_reference == field.type_reference
            && field.root == facts::PlaceRoot::Expression(captured.expression)
        {
            captured.root
        } else if let Some(receiver) = field.root.as_expression().and_then(|anchor| {
            self.dispatch_receivers
                .get(&(anchor.arena_index(), anchor.generation()))
        }) {
            // The dispatch has already chosen a completed value or an existing
            // place. A field read consumes that exact role, not its supplying
            // expression, which could move an affine field or replay a call.
            if !validation::declared_carriers_match(
                self.program,
                receiver.type_reference,
                field.type_reference,
            ) {
                return None;
            }
            receiver.value
        } else if field.receiver_value.is_valid() {
            if field.root.as_expression().is_some_and(|anchor| {
                matches!(self.program.expression_table.expression(anchor), ExpressionNode::SavedSubject(saved)
                    if matches!(self.program.expression_table.value_capture(saved.capture).owner,
                        typed_trees::expression::ValueCaptureOwner::Match))
            }) {
                // An unproduced Match alias cannot recover a receiver by
                // evaluating the original subject at this later observation.
                return None;
            }
            let value = self.structural_value(field.receiver_value, field.type_reference)?;
            self.retain_structural_value_root(value, field.type_reference)?;
            value
        } else {
            checked_trees::CheckedStructuralValueHandle::invalid()
        };
        let source = match field.root {
            facts::PlaceRoot::Symbol(symbol) => self.structural_source(symbol),
            facts::PlaceRoot::Expression(expression) => SourcePlan::StructuralExpression {
                expression,
                logical_root_type_identity: None,
            },
            _ => return None,
        };
        Some(self.insert(
            field.primitive_type,
            ComputationKind::StructuralField {
                atomic_ordering: None,
                result_type: field.result_type_reference,
                source_expression: expression,
                receiver_expression: if expression.is_valid() {
                    match self.program.expression_table.expression(expression) {
                        ExpressionNode::Member(member) => self
                            .program
                            .saved_argument_source(member.receiver)
                            .unwrap_or(member.receiver),
                        ExpressionNode::Indexed(indexed) => indexed.collection,
                        _ => ExpressionHandle::invalid(),
                    }
                } else {
                    // An admitted coordinate names established formal
                    // storage without an authored source occurrence.
                    ExpressionHandle::invalid()
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
        ))
    }
}

/// The closed integer a value-parameter descriptor denotes. A saved expression
/// and a saved local are the same observation: erasure leaves no runtime place,
/// and the application's integer operand is the field. A path below that
/// descriptor is a different observation and keeps the ordinary borrow.
fn closed_descriptor_value(
    program: &dyn ::validation::CheckingInput,
    field: &validation::StructuralScalarField,
) -> Option<numerics::literals::IntegerLiteral> {
    if !field.path.is_empty()
        || !matches!(
            field.root,
            facts::PlaceRoot::Expression(_) | facts::PlaceRoot::Symbol(_)
        )
    {
        return None;
    }
    let (_, _, index) = descriptor_fields(program, field.type_reference)
        .into_iter()
        .find(|(_, descriptor, _)| descriptor.symbol == field.field)?;
    let literal = program
        .typed_program()
        .static_const_literal_from_type_reference(index)?;
    let landed_type = field.primitive_type.landed_landing()?;
    Some(literal.with_landing(numerics::literals::IntegerLanding {
        landed_type,
        domain: field.domain,
    }))
}

/// A descriptor whose application operand is a runtime value parameter. The
/// parameter place saves that operand as the field; other places save a
/// closed integer literal.
fn descriptor_parameter_symbol(
    program: &dyn ::validation::CheckingInput,
    field: &validation::StructuralScalarField,
) -> Option<symbols::SymbolHandle> {
    if !field.path.is_empty()
        || !matches!(
            field.root,
            facts::PlaceRoot::Expression(_) | facts::PlaceRoot::Symbol(_)
        )
    {
        return None;
    }
    let (_, _, index) = descriptor_fields(program, field.type_reference)
        .into_iter()
        .find(|(_, descriptor, _)| descriptor.symbol == field.field)?;
    match program.type_reference_table.type_reference(index) {
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
            Some(*symbol)
        }
        _ => None,
    }
}

/// Value-parameter descriptors of the carrier, after the reference shells a
/// borrowed local spells in front of the data application.
fn descriptor_fields(
    program: &dyn ::validation::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Vec<(
    &typed_trees::data::TypeParameter,
    &typed_trees::data::DataField,
    typed_trees::types::TypeReferenceHandle,
)> {
    let mut visited = 0u32;
    loop {
        if visited > 8 {
            break;
        }
        visited += 1;
        let Some(shell) = validation::reference_shell(program, reference) else {
            break;
        };
        reference = shell.referee;
    }
    program.data_application_value_fields(reference)
}
