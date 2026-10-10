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
                self.machine,
                self.state,
                self.declaration_parameters,
                self.declaration_bindings,
                expression,
            );
        }
        validation::structural_scalar_field(
            self.program,
            self.machine,
            self.state,
            u32::try_from(self.statement_index).ok()?,
            expression,
        )
    }

    pub(super) fn structural_field(
        &mut self,
        expression: ExpressionHandle,
        field: validation::StructuralScalarField,
    ) -> Option<CheckedScalarComputationHandle> {
        let receiver_value = if field.receiver_value.is_valid() {
            let value = self.structural_value(field.receiver_value, field.type_reference)?;
            self.structural_values
                .roots
                .append(checked_trees::CheckedStructuralValueRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    expression: field.receiver_value,
                    type_reference: field.type_reference,
                    root: value,
                });
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
