//! Bind a declaration's pure selector to the observations of its complete input.
//! A foreign expression handle identifies source meaning, not an invocation
//! binding. Only the supplying telescope can translate its scalar leaves.

use super::DeclaredInputRead;
use crate::{
    CheckingInput, RankSubjectCoordinate, ScopedArithmeticBinder, ScopedArithmeticBinding,
    ScopedArithmeticExpression, ScopedArithmeticValue,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

impl DeclaredInputRead {
    pub fn selector_value(
        &self,
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        read: &mut impl FnMut(&RankSubjectCoordinate) -> Option<ScopedArithmeticValue>,
    ) -> Option<ScopedArithmeticValue> {
        self.selector_value_at(program, expression, read, &mut Vec::new())
    }

    fn selector_value_at(
        &self,
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        read: &mut impl FnMut(&RankSubjectCoordinate) -> Option<ScopedArithmeticValue>,
        active: &mut Vec<ExpressionHandle>,
    ) -> Option<ScopedArithmeticValue> {
        if active.len() >= 64 || active.contains(&expression) {
            return None;
        }
        active.push(expression);
        let value = (|| {
            if let Some(place) = crate::canonical_place_from_expression(program, expression)
                && let Some(root) = place.root.as_symbol()
            {
                if let Some((_, input)) = self
                    .selector_inputs
                    .iter()
                    .find(|(formal, _)| *formal == root)
                {
                    let mut coordinate = input.clone();
                    coordinate.path.extend(place.segments);
                    return read(&coordinate);
                }
                if place.segments.is_empty()
                    && let Some((formal, actual)) = self
                        .selector_arguments
                        .iter()
                        .find(|(bound, _)| *bound == root)
                    && let ExpressionNode::Name(name) =
                        program.expression_table.expression(expression)
                    && name.symbol == *formal
                    && *actual != expression
                {
                    return self.selector_value_at(program, *actual, read, active);
                }
            }
            let node = program.expression_table.expression(expression);
            match node {
                ExpressionNode::Integer(_) => {
                    Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: self.selector_owner,
                        expression,
                        bindings: Vec::new(),
                    }))
                }
                ExpressionNode::Binary(binary) => {
                    if !crate::has_builtin_parameter_bound_expression_meaning(
                        program,
                        self.selector_owner,
                        &self.selector_parameters,
                        expression,
                    ) {
                        return None;
                    }
                    let mut bindings = Vec::new();
                    for operand in [binary.left, binary.right] {
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(operand),
                            value: self.selector_value_at(program, operand, read, active)?,
                        });
                    }
                    Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: self.selector_owner,
                        expression,
                        bindings,
                    }))
                }
                _ => None,
            }
        })();
        active.pop();
        value
    }
}
