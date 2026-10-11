//! Field observation consumes either established storage or a complete evaluated
//! receiver. Both keep the same declared projection; a fresh receiver is an
//! ordinary structural value dependency, never just its selected initializer.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    ExpressionNode,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

impl Builder<'_, '_> {
    pub(super) fn structural_scalar_field(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::StructuralScalarField> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        ) {
            return None;
        }
        if !self.declaration_parameters.is_empty() {
            return validation::structural_scalar_field_in_parameters(
                self.program,
                self.declaration_parameters,
                self.declaration_bindings,
                expression,
            );
        }
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
            if self
                .structural_values
                .root_for_expression(self.state, statement, field.receiver_value)
                .is_none()
            {
                self.structural_values
                    .roots
                    .append(checked_trees::CheckedStructuralValueRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: statement,
                        expression: field.receiver_value,
                        type_reference: field.type_reference,
                        root: value,
                    });
            }
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
                source_expression: expression,
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
