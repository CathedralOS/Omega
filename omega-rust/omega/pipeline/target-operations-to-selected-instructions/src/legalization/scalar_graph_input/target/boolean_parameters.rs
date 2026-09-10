//! Replay parameter predicates simplified through exact Boolean constants.
use super::*;

impl Checker<'_> {
    pub(super) fn parameter_predicate(
        &self,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
        visited: &mut Vec<ValueId>,
    ) -> Option<(ValueId, bool)> {
        let value = resolve(value, aliases);
        if visited.contains(&value) {
            return None;
        }
        visited.push(value);
        if self.scalar_parameters().iter().any(|parameter| {
            parameter.value == value && parameter.scalar_type == ScalarType::Boolean
        }) {
            return Some((value, false));
        }
        let operation = &self
            .optimized
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find(|node| {
                node.definitions
                    .iter()
                    .any(|definition| definition.value == value)
            })?
            .operation;
        match *operation {
            AbstractOperation::BooleanNot { operand, .. } => {
                let (base, inverted) = self.parameter_predicate(operand, aliases, visited)?;
                Some((base, !inverted))
            }
            AbstractOperation::BooleanEqual { left, right, .. } => {
                let (operand, literal) = if let Some(literal) = self.boolean_literal(left, aliases)
                {
                    (right, literal)
                } else {
                    (left, self.boolean_literal(right, aliases)?)
                };
                let (base, inverted) = self.parameter_predicate(operand, aliases, visited)?;
                Some((base, inverted ^ !literal))
            }
            _ => None,
        }
    }

    fn boolean_literal(&self, value: ValueId, aliases: &[(ValueId, ValueId)]) -> Option<bool> {
        let value = resolve(value, aliases);
        self.optimized
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find_map(|node| match node.operation {
                AbstractOperation::BooleanConstant {
                    result,
                    value: literal,
                    ..
                } if result == value => Some(literal),
                _ => None,
            })
    }
}
