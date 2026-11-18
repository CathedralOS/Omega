//! Bounds on mathematical sums retain each operand's own integer type.

use super::*;
use proof_admission::ClosedIntegerEvaluator;
use semantic_vocabulary::IntegerMathLiteral;

impl Prover<'_> {
    pub(super) fn mathematical_sum_image(
        &mut self,
        first: &IntegerMathTerm,
        second: &IntegerMathTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let first = self.arithmetic_image(first, direction)?;
        let second = self.arithmetic_image(second, direction)?;
        let (Proposition::IntegerMathLessOrEqual(a, b), Proposition::IntegerMathLessOrEqual(c, d)) =
            (&first.conclusion, &second.conclusion)
        else {
            return Err("sum operand bounds are not mathematical orders");
        };
        let add = |left: &IntegerMathTerm, right: &IntegerMathTerm| {
            IntegerMathTerm::Add(Box::new(left.clone()), Box::new(right.clone()))
        };
        let left = add(a, c);
        let middle = add(b, c);
        let right = add(b, d);
        let first_goal = Proposition::IntegerMathLessOrEqual(left.clone(), middle.clone());
        let second_goal = Proposition::IntegerMathLessOrEqual(middle, right.clone());
        let (c, b) = (c.clone(), b.clone());
        let first = self
            .translate_mathematical_order(first, c, &first_goal)
            .ok_or("the kernel cannot translate the first sum bound")?;
        let second = self
            .translate_mathematical_order(second, b, &second_goal)
            .ok_or("the kernel cannot translate the second sum bound")?;
        let image = node(
            Proposition::IntegerMathLessOrEqual(left, right),
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(first),
                middle_less_or_equal_right: Box::new(second),
            },
        );
        Ok(self.normalize_image_endpoint(image, direction))
    }

    /// Keep the exact authored mathematical relation. A strict upper bound
    /// supplies its predecessor; no fit premise is invented from the goal.
    pub(super) fn mathematical_relation_image(
        &self,
        term: &IntegerMathTerm,
        direction: Direction,
        preferred: Option<&IntegerMathTerm>,
    ) -> Option<ProofNode> {
        let mut fallback = None;
        for fact in &self.facts {
            let (left, right, strict) = match &fact.proposition {
                Proposition::IntegerMathLessOrEqual(left, right) => (left, right, false),
                Proposition::IntegerMathLessThan(left, right) => (left, right, true),
                _ => continue,
            };
            let actual = match direction {
                Direction::Upper => left,
                Direction::Lower => right,
            };
            if compare_integer_math_terms(actual, term).ok().flatten() != Some(Ordering::Equal) {
                continue;
            }
            let mut relation = fact.proof(&self.facts);
            let one = IntegerMathTerm::literal(IntegerValue::Unsigned(1));
            if strict {
                relation = node(
                    Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::Add(Box::new(left.clone()), Box::new(one.clone())),
                        right.clone(),
                    ),
                    ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(relation),
                    },
                );
            }
            let (addend, goal) = match (direction, strict) {
                (Direction::Upper, true) => (
                    IntegerMathTerm::literal(IntegerValue::Signed(-1)),
                    Proposition::IntegerMathLessOrEqual(
                        term.clone(),
                        IntegerMathTerm::Subtract(Box::new(right.clone()), Box::new(one)),
                    ),
                ),
                (Direction::Lower, true) => (
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                    Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::Add(Box::new(left.clone()), Box::new(one)),
                        term.clone(),
                    ),
                ),
                (Direction::Upper, false) => (
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                    Proposition::IntegerMathLessOrEqual(term.clone(), right.clone()),
                ),
                (Direction::Lower, false) => (
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                    Proposition::IntegerMathLessOrEqual(left.clone(), term.clone()),
                ),
            };
            let Some(candidate) = self.translate_mathematical_order(relation, addend, &goal) else {
                continue;
            };
            let candidate = self.normalize_image_endpoint(candidate, direction);
            if preferred.is_some_and(|limit| image_satisfies(&candidate, limit, direction)) {
                return Some(candidate);
            }
            if fallback.is_none() {
                fallback = Some(candidate);
            }
        }
        fallback
    }

    fn normalize_image_endpoint(&self, image: ProofNode, direction: Direction) -> ProofNode {
        let (left, right, endpoint) = match (&image.conclusion, direction) {
            (Proposition::IntegerMathLessOrEqual(left, right), Direction::Upper) => {
                (left, right, 1)
            }
            (Proposition::IntegerMathLessOrEqual(left, right), Direction::Lower) => {
                (left, right, 0)
            }
            _ => return image,
        };
        let actual = if endpoint == 0 { left } else { right };
        let Some(value) = ClosedIntegerEvaluator::default()
            .evaluate_closed(actual)
            .ok()
            .flatten()
        else {
            return image;
        };
        let Some(magnitude) = value.abs().to_u128() else {
            return image;
        };
        let Ok(literal) = IntegerMathLiteral::new(value.is_negative(), magnitude) else {
            return image;
        };
        let literal = IntegerMathTerm::IntegerLiteral(literal);
        if &literal == actual {
            return image;
        }
        let Some(equality) = self.closed(sorted_math_equal(actual.clone(), literal.clone())) else {
            return image;
        };
        let goal = if endpoint == 0 {
            Proposition::IntegerMathLessOrEqual(literal, right.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(left.clone(), literal)
        };
        node(
            goal,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(image),
                equality: Box::new(equality),
                endpoint,
            },
        )
    }
}

pub(super) fn image_satisfies(
    image: &ProofNode,
    limit: &IntegerMathTerm,
    direction: Direction,
) -> bool {
    let bound = match (&image.conclusion, direction) {
        (Proposition::IntegerMathLessOrEqual(_, bound), Direction::Upper)
        | (Proposition::IntegerMathLessOrEqual(bound, _), Direction::Lower) => bound,
        _ => return false,
    };
    compare_integer_math_terms(bound, limit)
        .ok()
        .flatten()
        .is_some_and(|ordering| match direction {
            Direction::Upper => ordering.is_le(),
            Direction::Lower => ordering.is_ge(),
        })
}
