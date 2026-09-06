//! Structural Boolean consequences of strictly checked entry predicates only.
//! No branch is discarded as infeasible, and facts from different alternatives
//! are intersected rather than joined as simultaneous assumptions.

use super::*;

pub(super) struct Consequences<'a> {
    program: &'a TypedTrees,
    parameter_names: &'a [String],
    content_conservation: &'a [validation::ContentConservationSourcePlan],
    remaining: usize,
}

enum Structure {
    Operand(ExpressionHandle, bool),
    Pair(ExpressionHandle, ExpressionHandle, bool),
    Atom,
}

impl<'a> Consequences<'a> {
    pub(super) fn new(
        program: &'a TypedTrees,
        parameter_names: &'a [String],
        content_conservation: &'a [validation::ContentConservationSourcePlan],
    ) -> Self {
        Self {
            program,
            parameter_names,
            content_conservation,
            remaining: 4096,
        }
    }

    fn charge(&mut self, steps: usize) -> Option<()> {
        match self.remaining.checked_sub(steps) {
            Some(remaining) => {
                self.remaining = remaining;
                Some(())
            }
            None => {
                self.remaining = 0;
                None
            }
        }
    }

    fn identity(&self, expression: ExpressionHandle, negated: bool) -> CrashPredicateIdentity {
        crate::facts::canonical_crash_path_predicate(
            self.program,
            expression,
            negated,
            self.parameter_names,
            self.content_conservation,
        )
    }

    pub(super) fn collect(
        &mut self,
        expression: ExpressionHandle,
        negated: bool,
        depth: usize,
    ) -> Option<Vec<CrashPredicateIdentity>> {
        if depth >= 64 {
            return None;
        }
        self.charge(1)?;
        let mut output = vec![self.identity(expression, negated)];
        match structure(self.program, expression, negated) {
            Structure::Operand(operand, polarity) => {
                output.extend(self.collect(operand, polarity, depth + 1)?);
            }
            Structure::Pair(left, right, conjunction) => {
                let mut left = self.collect(left, negated, depth + 1)?;
                let right = self.collect(right, negated, depth + 1)?;
                self.charge(left.len().checked_add(right.len())?)?;
                if conjunction {
                    left.extend(right);
                } else {
                    // Both recursively computed sets are sorted. Only exact
                    // identities proved in every alternative survive.
                    left.retain(|identity| right.binary_search(identity).is_ok());
                }
                output.extend(left);
            }
            Structure::Atom => {
                // Preserve the existing exact equality/reversed-operand atom
                // forms. Strict entry preflight excludes numeric operands and
                // selected authored operations before this reader is called.
                super::super::collect_structural_guard_consequences(
                    self.program,
                    expression,
                    negated,
                    self.parameter_names,
                    self.content_conservation,
                    &mut output,
                );
            }
        }
        self.charge(output.len())?;
        output.sort_unstable();
        output.dedup();
        Some(output)
    }

    pub(super) fn establishes(
        &mut self,
        expression: ExpressionHandle,
        negated: bool,
        known: &[CrashPredicateIdentity],
        depth: usize,
    ) -> Option<bool> {
        if depth >= 64 {
            return None;
        }
        self.charge(1)?;
        if known
            .binary_search(&self.identity(expression, negated))
            .is_ok()
        {
            return Some(true);
        }
        match structure(self.program, expression, negated) {
            Structure::Operand(operand, polarity) => {
                self.establishes(operand, polarity, known, depth + 1)
            }
            Structure::Pair(left, right, conjunction) => {
                let left = self.establishes(left, negated, known, depth + 1)?;
                if left != conjunction {
                    return Some(left);
                }
                self.establishes(right, negated, known, depth + 1)
            }
            Structure::Atom => Some(false),
        }
    }
}

fn structure(program: &TypedTrees, expression: ExpressionHandle, negated: bool) -> Structure {
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            Structure::Operand(unary.operand, !negated)
        }
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
        {
            Structure::Pair(
                binary.left,
                binary.right,
                (binary.operator == BinaryOperator::And) != negated,
            )
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let (operand, literal) = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => (binary.right, *literal),
                (_, ExpressionNode::Boolean(literal)) => (binary.left, *literal),
                _ => return Structure::Atom,
            };
            let equality_is_negated = negated != (binary.operator == BinaryOperator::NotEqual);
            Structure::Operand(operand, equality_is_negated == literal)
        }
        _ => Structure::Atom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_is_shared_and_cannot_resume_after_an_oversized_charge() {
        let program = TypedTrees::default();
        let mut consequences = Consequences::new(&program, &[], &[]);
        assert_eq!(consequences.charge(4090), Some(()));
        assert_eq!(consequences.charge(7), None);
        assert_eq!(consequences.remaining, 0);
        assert_eq!(consequences.charge(1), None);
        assert!(
            consequences
                .collect(ExpressionHandle::default(), false, 0)
                .is_none()
        );
        assert_eq!(
            consequences.establishes(ExpressionHandle::default(), false, &[], 0),
            None,
        );
    }

    #[test]
    fn depth_limit_applies_before_collecting_or_matching_an_identity() {
        let program = TypedTrees::default();
        let mut consequences = Consequences::new(&program, &[], &[]);
        assert!(
            consequences
                .collect(ExpressionHandle::default(), false, 64)
                .is_none()
        );
        assert_eq!(
            consequences.establishes(ExpressionHandle::default(), false, &[], 64),
            None,
        );
    }
}
