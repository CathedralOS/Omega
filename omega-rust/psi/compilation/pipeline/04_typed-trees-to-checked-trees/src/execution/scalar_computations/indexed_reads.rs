//! Scalar and sequence-length observations share declaration geometry with pure reads.
//! A call-bearing selector is an ordinary computation operand of the read's
//! existing Apply node. Completing those operands left-to-right before the
//! read preserves their effects and once-only schedule without treating a
//! selector invocation as a second statement call. The indexing occurrence
//! still owns its source-checked bound; an operand result supplies only bits.
use super::{
    ArithmeticDomain, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarExpression, ExpressionHandle, PrimitiveType,
};
use crate::values::scalar::expression_facts::{is_integer, scalar_expression_type};

impl Builder<'_, '_> {
    pub(super) fn indexed_read(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<(
        CheckedScalarComputationHandle,
        ArithmeticDomain,
        PrimitiveType,
    )> {
        let program = self.program;
        let operators = self.operators;
        let parameters = self.authored_parameters;
        let mut operands = Vec::new();
        let mut selector = |_, selector| {
            let operand = self.integer_operand(selector, Some(PrimitiveType::U64))?;
            let primitive_type = scalar_expression_type(&operand.value)?;
            if !is_integer(primitive_type) || primitive_type == PrimitiveType::Addr {
                return None;
            }
            let position = operands.len();
            operands.push(self.materialize_integer(operand)?);
            Some(CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            })
        };
        let (template, domain) = if program
            .expression_table
            .authored_length_receiver(expression)
            .is_some()
        {
            (
                crate::values::scalar::structural_fields::structural_sequence_length(
                    program,
                    operators,
                    parameters,
                    expression,
                    &mut selector,
                )?,
                ArithmeticDomain::Exact,
            )
        } else {
            crate::values::scalar::scalar_lowering::indexed_scalar_read_with_selectors(
                program, operators, expression, parameters, selector,
            )?
        };
        let primitive_type = scalar_expression_type(&template)?;
        let operands = self.plans.operands.insert_many(operands);
        let computation = self.insert(
            primitive_type,
            CheckedScalarComputationKind::Apply {
                source_expression: expression,
                expression: template,
                operands,
            },
        );
        Some((computation, domain, primitive_type))
    }
}
