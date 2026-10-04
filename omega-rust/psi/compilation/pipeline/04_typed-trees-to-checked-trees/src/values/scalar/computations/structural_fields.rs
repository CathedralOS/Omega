//! Established structural observations are leaves in the ordinary evaluation sequence.
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
        let source = match field.root {
            facts::PlaceRoot::Symbol(symbol) => {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            }
            facts::PlaceRoot::Expression(expression) => {
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression }
            }
            _ => return None,
        };
        Some(
            self.insert(
                field.primitive_type,
                CheckedScalarComputationKind::StructuralField {
                    source_expression: expression,
                    subject: CheckedUnitStructuralArgumentPlan {
                        source,
                        path: field.path,
                        type_identity: self
                            .program
                            .normalized_type_identity(field.carrier_type_reference)
                            .into_string(),
                        access: CheckedStructuralAccess::SharedBorrow,
                    },
                    field: field.field,
                },
            ),
        )
    }
}
