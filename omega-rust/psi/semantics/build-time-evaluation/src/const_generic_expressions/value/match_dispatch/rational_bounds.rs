//! All-arm nonzero evidence for anonymous arithmetic containing dispatch.
//!
//! A closed rational interval contains every result, regardless of which arm
//! executes. Joins lose correlations deliberately: containing zero is an
//! undischarged obligation, not evidence that execution actually divides by zero.
//! This is not integer landing and proves neither integrality nor carrier fit.
//! Integer interval analysis has different division and width semantics, so only
//! its interval laws apply here; all arithmetic uses the shared exact rationals.
//!
//! The caller has validated the complete acyclic scalar graph and checks every
//! subject, pattern, and arm. This pass visits only anonymous result edges; it
//! cannot execute landed operations in an undemanded subject or select an arm.
//! Each binary composes two bounds, never combinations of branch selections.
//! Both children are checked even when multiplication by zero would erase their
//! result, since an undefined anonymous subexpression has no numeric value.

use numerics::bignum::BigRational;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
};
use validation::evaluate_anonymous_numeric_expression_with_selected_match_arms;

pub(super) fn excludes_zero(
    program: &TypedTrees,
    root: ExpressionHandle,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Result<bool, String> {
    enum Step {
        Enter(ExpressionHandle),
        Binary(BinaryOperator),
        Join(usize),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut values: Vec<RationalBounds> = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => match program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    let arms = program.expression_table.match_arms(dispatch.arms);
                    if arms.is_empty() || arms.len() != dispatch.arms.len() {
                        return Err("anonymous rational bounds require valid Match arms".into());
                    }
                    pending.push(Step::Join(arms.len()));
                    pending.extend(arms.iter().rev().map(|arm| Step::Enter(arm.value)));
                }
                ExpressionNode::Binary(binary) => {
                    if !builtin(expression) {
                        return Err(
                            "anonymous rational bounds require selected builtin meaning".into()
                        );
                    }
                    pending.push(Step::Binary(binary.operator));
                    pending.push(Step::Enter(binary.right));
                    pending.push(Step::Enter(binary.left));
                }
                ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                    let value = evaluate_anonymous_numeric_expression_with_selected_match_arms(
                        program,
                        expression,
                        &[],
                        &mut builtin,
                    )
                    .ok_or("anonymous rational bounds require exact anonymous literals")?;
                    values.push(RationalBounds::constant(value));
                }
                _ => return Err("unsupported expression in anonymous rational bounds".into()),
            },
            Step::Binary(operator) => {
                let right = values
                    .pop()
                    .ok_or("missing right anonymous rational bounds")?;
                let left = values
                    .pop()
                    .ok_or("missing left anonymous rational bounds")?;
                values.push(left.apply(operator, right)?);
            }
            Step::Join(count) => {
                let mut joined = values
                    .pop()
                    .ok_or("missing anonymous Match result bounds")?;
                for _ in 1..count {
                    joined.include(values.pop().ok_or("missing anonymous Match arm bounds")?);
                }
                values.push(joined);
            }
        }
    }
    if values.len() != 1 {
        return Err("anonymous arithmetic did not produce one rational range".into());
    }
    Ok(values
        .pop()
        .ok_or("missing anonymous rational bounds")?
        .excludes_zero())
}

struct RationalBounds {
    low: BigRational,
    high: BigRational,
}

impl RationalBounds {
    fn constant(value: BigRational) -> Self {
        Self {
            low: value.clone(),
            high: value,
        }
    }

    fn excludes_zero(&self) -> bool {
        self.low.cmp_value(&BigRational::zero()).is_gt()
            || self.high.cmp_value(&BigRational::zero()).is_lt()
    }

    fn include(&mut self, other: Self) {
        if other.low.cmp_value(&self.low).is_lt() {
            self.low = other.low;
        }
        if other.high.cmp_value(&self.high).is_gt() {
            self.high = other.high;
        }
    }

    fn corners([first, second, third, fourth]: [BigRational; 4]) -> Self {
        let mut bounds = Self::constant(first);
        for value in [second, third, fourth] {
            bounds.include(Self::constant(value));
        }
        bounds
    }

    fn apply(self, operator: BinaryOperator, right: Self) -> Result<Self, String> {
        Ok(match operator {
            BinaryOperator::Add => Self {
                low: self.low.add(&right.low),
                high: self.high.add(&right.high),
            },
            BinaryOperator::Subtract => Self {
                low: self.low.sub(&right.high),
                high: self.high.sub(&right.low),
            },
            BinaryOperator::Multiply => Self::corners([
                self.low.mul(&right.low),
                self.low.mul(&right.high),
                self.high.mul(&right.low),
                self.high.mul(&right.high),
            ]),
            BinaryOperator::Divide => {
                if !right.excludes_zero() {
                    return Err("anonymous rational division requires a nonzero divisor proof; its rational bounds include zero".into());
                }
                // On a zero-free denominator interval the quotient extrema
                // occur at corners, also for negative or fractional endpoints.
                let divide = |left: &BigRational, right: &BigRational| {
                    left.div(right)
                        .ok_or("undefined anonymous rational quotient")
                };
                Self::corners([
                    divide(&self.low, &right.low)?,
                    divide(&self.low, &right.high)?,
                    divide(&self.high, &right.low)?,
                    divide(&self.high, &right.high)?,
                ])
            }
            _ => return Err("anonymous rational bounds require arithmetic meaning".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::bignum::BigInt;

    fn fraction(numerator: i64, denominator: i64) -> BigRational {
        BigRational::from_integer(BigInt::from_i64(numerator))
            .div(&BigRational::from_integer(BigInt::from_i64(denominator)))
            .expect("nonzero test denominator")
    }

    #[test]
    fn rational_bounds_contain_interior_and_endpoint_arithmetic() {
        for left_low in -2..=2 {
            for left_high in left_low..=2 {
                for right_low in -2..=2 {
                    for right_high in right_low..=2 {
                        for operator in [
                            BinaryOperator::Add,
                            BinaryOperator::Subtract,
                            BinaryOperator::Multiply,
                            BinaryOperator::Divide,
                        ] {
                            let left = RationalBounds {
                                low: fraction(left_low, 2),
                                high: fraction(left_high, 2),
                            };
                            let right = RationalBounds {
                                low: fraction(right_low, 2),
                                high: fraction(right_high, 2),
                            };
                            let result = left.apply(operator, right);
                            if operator == BinaryOperator::Divide
                                && right_low <= 0
                                && right_high >= 0
                            {
                                assert!(result.is_err(), "zero-containing denominator interval");
                                continue;
                            }
                            let bounds = result.expect("defined rational bounds");
                            for left in [
                                fraction(left_low, 2),
                                fraction(left_low + left_high, 4),
                                fraction(left_high, 2),
                            ] {
                                for right in [
                                    fraction(right_low, 2),
                                    fraction(right_low + right_high, 4),
                                    fraction(right_high, 2),
                                ] {
                                    let actual = match operator {
                                        BinaryOperator::Add => left.add(&right),
                                        BinaryOperator::Subtract => left.sub(&right),
                                        BinaryOperator::Multiply => left.mul(&right),
                                        BinaryOperator::Divide => {
                                            left.div(&right).expect("zero-free interval")
                                        }
                                        _ => unreachable!(),
                                    };
                                    assert!(!bounds.low.cmp_value(&actual).is_gt());
                                    assert!(!bounds.high.cmp_value(&actual).is_lt());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rational_bounds_visit_result_operations_once_without_subject_execution() {
        use source_files_to_tokens::Lexer;
        use typed_trees::statement::StatementNode;

        let term = "(match (1u8 / 0 == 0) { true -> 1, false -> 2 })";
        let expression = std::iter::repeat_n(term, 24)
            .collect::<Vec<_>>()
            .join(" + ");
        let source = format!("machine choose() -> u8 {{ {expression} }}");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(
            &Lexer::new(&source).tokenize().expect("tokens"),
        )
        .expect("syntax");
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typed");
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Expression(root) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("expression fixture");
        };
        super::super::validate_graph(&program, root).expect("validated caller precondition");
        let mut visits = 0;
        assert!(excludes_zero(&program, root, |expression| {
            visits += 1;
            assert!(matches!(program.expression_table.expression(expression), ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Add));
            validation::has_builtin_binary_expression_meaning(&program, machine, Some(state), expression)
        }).expect("positive sum"));
        assert_eq!(
            visits, 23,
            "one visit per addition, no branch combinations or subject operations"
        );
        assert!(
            excludes_zero(&program, root, |_| false).is_err(),
            "token spelling grants no builtin authority"
        );
    }
}
