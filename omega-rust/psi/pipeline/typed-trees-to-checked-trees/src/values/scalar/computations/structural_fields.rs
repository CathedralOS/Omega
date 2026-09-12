//! Local record observations are leaves in the ordinary evaluation sequence.

use super::*;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

impl Builder<'_, '_> {
    pub(super) fn local_scalar_record_field(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::LocalScalarRecordField> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        ) {
            return None;
        }
        validation::local_scalar_record_field(
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
        field: validation::LocalScalarRecordField,
    ) -> CheckedScalarComputationHandle {
        self.insert(
            field.primitive_type,
            CheckedScalarComputationKind::StructuralField {
                source_expression: expression,
                subject: CheckedUnitStructuralArgumentPlan {
                    source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: field.local,
                    },
                    path: Vec::new(),
                    type_identity: self
                        .program
                        .normalized_type_identity(field.type_reference)
                        .into_string(),
                    access: CheckedStructuralAccess::SharedBorrow,
                },
                field: field.field,
            },
        )
    }
}
