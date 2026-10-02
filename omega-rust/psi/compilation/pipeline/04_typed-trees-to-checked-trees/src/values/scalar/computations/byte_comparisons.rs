//! Content comparison observes a completed structural operand. Pure places
//! and computed descriptors share the same query; descriptor establishment
//! stays at the operand's selective evaluation point.
use super::{
    BinaryOperator, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    ExpressionHandle, ExpressionNode, PrimitiveType, semantic_casts,
};
use crate::execution::terminal_unit::{calls, types};
use checked_trees::{
    CheckedStorageRoot, CheckedStructuralAccess,
    CheckedStructuralPredicatePathSegment as Predicate, CheckedSubsliceSite,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan as Source,
    CheckedUnitStructuralPathSegment as Path,
};

impl Builder<'_, '_> {
    pub(super) fn byte_literal_comparison(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) || !super::operator_is_builtin(self.operators, expression)
        {
            return None;
        }
        let operand = match (
            self.program.expression_table.expression(binary.left),
            self.program.expression_table.expression(binary.right),
        ) {
            (_, ExpressionNode::String(_)) => binary.left,
            (ExpressionNode::String(_), _) => binary.right,
            _ => return None,
        };
        let target = semantic_casts::result_type(self.program, self.state, operand)?;
        super::super::boolean_lowering::comparable_byte_carrier_type(self.program, target)?;
        let (machine, state) =
            crate::semantic::calls::find_state_with_machine(self.program, self.state)?;
        let subject = if let Some((root, path, _)) =
            super::super::boolean_lowering::comparable_byte_carrier(
                self.program,
                self.authored_parameters,
                operand,
            ) {
            let source = match root {
                CheckedStorageRoot::Parameter { index } => Source::Parameter {
                    parameter_index: self.dense_structural_parameter_index(index as usize)?,
                },
                CheckedStorageRoot::OwnedLocal { symbol }
                | CheckedStorageRoot::ViewLocal { symbol } => Source::StructuralLocal { symbol },
            };
            let path = path
                .into_iter()
                .map(|segment| match segment {
                    Predicate::Field(field) => Some(Path::Field(field)),
                    Predicate::FixedIndex(index) => Some(Path::FixedIndex(index)),
                    Predicate::RuntimeIndex(index) => Some(Path::RuntimeIndex(index)),
                    Predicate::Case(_) => None,
                })
                .collect::<Option<Vec<_>>>()?;
            CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: types::byte_sequence_type_identity(self.program, target, &[], &[])?,
                access: CheckedStructuralAccess::SharedBorrow,
            }
        } else {
            let parameters = if machine.attached_data.is_some() {
                calls::ambient_self_scalar_graph_signature(self.program, machine, state)?.0
            } else {
                calls::free_structural_scalar_signature(
                    self.program,
                    &mut types::ShapeCollector::new(self.program),
                    state,
                    &[],
                )?
                .0
            };
            calls::view_subslice::admit_resolved(
                self.program,
                self.operators,
                self.pure,
                machine,
                state,
                &parameters,
                target,
                operand,
                self.statement_index,
                CheckedSubsliceSite::Expression {
                    expression: operand,
                },
            )?
            .argument()
        };
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                operand_type: target,
                source_expression: expression,
                subject,
            },
        ))
    }
}
