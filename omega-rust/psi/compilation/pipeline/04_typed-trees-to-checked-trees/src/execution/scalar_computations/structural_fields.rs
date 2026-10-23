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

pub(super) struct PrimitiveReferenceObservation {
    pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
    pub(super) primitive_type: typed_trees::types::PrimitiveType,
    pub(super) domain: numerics::arithmetic::ArithmeticDomain,
}

impl Builder<'_, '_> {
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
        Some(self.insert(
            expected,
            CheckedScalarComputationKind::StructuralField {
                atomic_ordering: None,
                result_type: validation::reference_shell(self.program, reference)?.referee,
                source_expression: expression,
                receiver_expression: expression,
                source_type: reference,
                receiver_value,
                subject: CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                    },
                    path: vec![checked_trees::CheckedUnitStructuralPathSegment::Referent],
                    type_identity: validation::stored_structural_type_identity(
                        self.program,
                        reference,
                        &[],
                        &[],
                    ),
                    access: CheckedStructuralAccess::SharedBorrow,
                },
                field: symbols::SymbolHandle::invalid(),
                field_identity: Default::default(),
            },
        ))
    }

    /// Receiving positions and arithmetic operands ask the same type question.
    /// The reference's declared referee supplies its scalar carrier and policy;
    /// neither a surrounding operator nor the return ABI may invent them.
    pub(super) fn primitive_reference_observation_type(
        &self,
        expression: ExpressionHandle,
    ) -> Option<PrimitiveReferenceObservation> {
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
        let shell = validation::reference_shell(self.program, reference)?;
        if !shell.access.is_readable() {
            return None;
        }
        Some(PrimitiveReferenceObservation {
            type_reference: reference,
            primitive_type: self.program.primitive_type_reference(shell.referee)?,
            domain: self
                .program
                .arithmetic_domain_for_type_reference(shell.referee),
        })
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
        self.structural_receiver_scalar(expression)
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
        let receiver_value = if field.receiver_value.is_valid() {
            let value = self.structural_value(field.receiver_value, field.type_reference)?;
            let statement = u32::try_from(self.statement_index).ok()?;
            self.retain_structural_root(
                statement,
                field.receiver_value,
                field.type_reference,
                value,
            );
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
        Some(self.insert(
            field.primitive_type,
            CheckedScalarComputationKind::StructuralField {
                atomic_ordering: None,
                result_type: validation::expression_type_reference_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    expression,
                )?,
                source_expression: expression,
                receiver_expression: match self.program.expression_table.expression(expression) {
                    ExpressionNode::Member(member) => member.receiver,
                    ExpressionNode::Indexed(indexed) => indexed.collection,
                    _ => ExpressionHandle::invalid(),
                },
                source_type: field.type_reference,
                receiver_value,
                subject: CheckedUnitStructuralArgumentPlan {
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
