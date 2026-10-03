//! Content comparison observes a completed structural operand. Pure places
//! and computed descriptors share the same query; descriptor establishment
//! stays at the operand's selective evaluation point.
use super::{
    BinaryOperator, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    ExpressionHandle, ExpressionNode, PrimitiveType, semantic_casts,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

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
        let subject = self.byte_operand_subject(operand, target)?;
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                operand_type: target,
                source_expression: expression,
                subject,
            },
        ))
    }

    /// A declared byte-predicate call observes the same completed structural
    /// operand a content comparison would: a pure place or a computed
    /// descriptor admitted at the same selective evaluation point. The
    /// predicate's reads happen only after the operand's own preparation.
    pub(super) fn byte_predicate(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let (predicate, operand) =
            typed_trees::byte_predicates::byte_predicate_call(self.program, expression)?;
        let target = semantic_casts::result_type(self.program, self.state, operand)?;
        super::super::boolean_lowering::comparable_byte_carrier_type(self.program, target)?;
        let subject = self.byte_operand_subject(operand, target)?;
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::ByteSequencePredicate {
                predicate,
                operand_type: target,
                source_expression: expression,
                subject,
            },
        ))
    }

    /// Observations and descriptor stores prepare the same byte operand.
    fn byte_operand_subject(
        &mut self,
        operand: ExpressionHandle,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let (machine, state) =
            crate::semantic::calls::find_state_with_machine(self.program, self.state)?;
        crate::execution::terminal_unit::byte_operands::plan(
            self.program,
            self.operators,
            self.pure,
            machine,
            state,
            self.statement_index,
            operand,
            target,
            &|position| self.dense_structural_parameter_index(position),
            &|selector| self.local_selector(selector),
        )
    }
}
