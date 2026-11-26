//! Bounds on mathematical sums retain each operand's own integer type.

use super::*;
use proof_admission::ClosedIntegerEvaluator;
use semantic_vocabulary::IntegerMathLiteral;

impl Prover<'_> {
    /// An offset with a checked sign orders a base and its translated endpoint.
    /// The kernel checks the sign and the translation on mathematical integers.
    pub(super) fn mathematical_offset_order(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Option<ProofNode> {
        let mut pending = left.operands();
        pending.extend(right.operands());
        while let Some(offset) = pending.pop() {
            pending.extend(offset.operands());
            if offset == left || offset == right {
                continue;
            }
            for (base, sum, extend_right) in [(left, right, true), (right, left, false)] {
                let translated = IntegerMathTerm::Add(Box::new(base.clone()), Box::new(offset.clone()));
                if compare_integer_math_terms(&translated, sum).ok().flatten() != Some(Ordering::Equal) {
                    continue;
                }
                let zero = IntegerMathTerm::literal(IntegerValue::Unsigned(0));
                let bound = if extend_right {
                    Proposition::IntegerMathLessOrEqual(zero, offset.clone())
                } else {
                    Proposition::IntegerMathLessOrEqual(offset.clone(), zero)
                };
                let Ok(proof) = self.prove(&bound) else { continue };
                if let Some(translated) = self.translate_mathematical_order(
                    proof, base.clone(),
                    &Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
                ) {
                    return Some(translated);
                }
            }
        }
        None
    }

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
        for index in 0..self.facts.len() {
            let Some(candidate) = self.mathematical_fact_image(index, term, direction) else {
                continue;
            };
            if preferred.is_some_and(|limit| image_satisfies(&candidate, limit, direction)) {
                return Some(candidate);
            }
            if fallback.is_none() {
                fallback = Some(candidate);
            }
        }
        fallback
    }

    fn mathematical_fact_image(
        &self,
        index: usize,
        term: &IntegerMathTerm,
        direction: Direction,
    ) -> Option<ProofNode> {
        let fact = &self.facts[index];
        let (left, right, strict) = match fact.proposition() {
            Proposition::IntegerMathLessOrEqual(left, right) => (left, right, false),
            Proposition::IntegerMathLessThan(left, right) => (left, right, true),
            _ => return None,
        };
        let actual = match direction {
            Direction::Upper => left,
            Direction::Lower => right,
        };
        if compare_integer_math_terms(actual, term).ok().flatten() != Some(Ordering::Equal) {
            return None;
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
        self.translate_mathematical_order(relation, addend, &goal)
            .map(|image| self.normalize_image_endpoint(image, direction))
    }

    /// A retained bound on base + capacity also bounds base + requested when
    /// the actual requested count is smaller. Every order and endpoint rewrite
    /// is ordinary checked evidence; the bound itself is never assumed.
    pub(super) fn mathematical_composed_image(
        &mut self,
        term: &IntegerMathTerm,
        direction: Direction,
        preferred: Option<&IntegerMathTerm>,
    ) -> Option<ProofNode> {
        let mut fallback = None;
        for index in 0..self.facts.len() {
            let actual = match (self.facts[index].proposition(), direction) {
                (
                    Proposition::IntegerMathLessOrEqual(left, _)
                    | Proposition::IntegerMathLessThan(left, _),
                    Direction::Upper,
                ) => left,
                (
                    Proposition::IntegerMathLessOrEqual(_, right)
                    | Proposition::IntegerMathLessThan(_, right),
                    Direction::Lower,
                ) => right,
                _ => continue,
            };
            if !matches!(actual, IntegerMathTerm::Add(..)) || actual == term {
                continue;
            }
            let actual = actual.clone();
            let Some(existing) = self.mathematical_fact_image(index, &actual, direction) else {
                continue;
            };
            let (left, right) = match direction {
                Direction::Upper => (term, &actual),
                Direction::Lower => (&actual, term),
            };
            let bridge = self
                .common_sum_order(left, right, direction == Direction::Upper)
                .or_else(|| {
                    if !preferred.is_some_and(|limit| image_satisfies(&existing, limit, direction))
                    {
                        return None;
                    }
                    self.translated_equality_order(left, right)
                        .map(|proof| (proof, false))
                });
            let Some((bridge, strict)) = bridge else {
                continue;
            };
            let Proposition::IntegerMathLessOrEqual(first, middle) = &bridge.conclusion else {
                continue;
            };
            let Proposition::IntegerMathLessOrEqual(bound_left, bound_right) = &existing.conclusion
            else {
                continue;
            };
            let (goal, first_proof, second_proof) = match direction {
                Direction::Upper if middle == bound_left => (
                    Proposition::IntegerMathLessOrEqual(first.clone(), bound_right.clone()),
                    bridge,
                    existing,
                ),
                Direction::Lower if bound_right == first => (
                    Proposition::IntegerMathLessOrEqual(bound_left.clone(), middle.clone()),
                    existing,
                    bridge,
                ),
                _ => continue,
            };
            let mut image = node(
                goal,
                ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(first_proof),
                    middle_less_or_equal_right: Box::new(second_proof),
                },
            );
            if strict {
                let Proposition::IntegerMathLessOrEqual(_, bound) = &image.conclusion else {
                    continue;
                };
                let goal = Proposition::IntegerMathLessOrEqual(
                    term.clone(),
                    IntegerMathTerm::Subtract(
                        Box::new(bound.clone()),
                        Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                    ),
                );
                let Some(translated) = self.translate_mathematical_order(
                    image,
                    IntegerMathTerm::literal(IntegerValue::Signed(-1)),
                    &goal,
                ) else {
                    continue;
                };
                image = translated;
            }
            let image = self.normalize_image_endpoint(image, direction);
            if preferred.is_some_and(|limit| image_satisfies(&image, limit, direction)) {
                return Some(image);
            }
            if fallback.is_none() {
                fallback = Some(image);
            }
        }
        fallback
    }

    /// If the two endpoints differ by a retained equality, translating that
    /// equality by their common offset preserves the bound. For example,
    /// `a + x == b` lets a bound on `b + y` bound `a + (x + y)`.
    fn translated_equality_order(
        &self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Option<ProofNode> {
        let goal = Proposition::IntegerMathLessOrEqual(left.clone(), right.clone());
        for fact in &self.facts {
            let lifted;
            let (a, b) = match fact.proposition() {
                Proposition::IntegerMathEqual(a, b) => (a, b),
                scalar @ Proposition::Equal(..) => {
                    lifted = proof_admission::lift_fixed_integer_relation(scalar);
                    let Some(Proposition::IntegerMathEqual(a, b)) = &lifted else {
                        continue;
                    };
                    (a, b)
                }
                _ => continue,
            };
            for (start, end) in [(a, b), (b, a)] {
                let offset =
                    IntegerMathTerm::Subtract(Box::new(left.clone()), Box::new(start.clone()));
                let order = Proposition::IntegerMathLessOrEqual(start.clone(), end.clone());
                let Some(translation) =
                    self.prepare_mathematical_order_translation(&order, offset, &goal)
                else {
                    continue;
                };
                let equality = fact.proof(&self.facts);
                let relation = if let Proposition::Equal(a, _) = &equality.conclusion {
                    let equality = if IntegerMathTerm::embed(a.clone()).as_ref() == Some(start) {
                        equality
                    } else {
                        flip(equality).ok()?
                    };
                    ranges::lift_endpoint_order(weaken(equality).ok()?)?
                } else {
                    let reflexive = self.closed(Proposition::IntegerMathLessOrEqual(
                        start.clone(),
                        start.clone(),
                    ))?;
                    node(
                        order,
                        ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(reflexive),
                            equality: Box::new(equality),
                            endpoint: 1,
                        },
                    )
                };
                return Some(translation.apply(relation));
            }
        }
        None
    }

    fn common_sum_order(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
        prefer_strict: bool,
    ) -> Option<(ProofNode, bool)> {
        let (IntegerMathTerm::Add(a, b), IntegerMathTerm::Add(c, d)) = (left, right) else {
            return None;
        };
        for (common, smaller) in [(a, b), (b, a)] {
            for (other_common, larger) in [(c, d), (d, c)] {
                if compare_integer_math_terms(common, other_common)
                    .ok()
                    .flatten()
                    != Some(Ordering::Equal)
                {
                    continue;
                }
                if prefer_strict {
                    let strict =
                        Proposition::IntegerMathLessThan((**smaller).clone(), (**larger).clone());
                    if let Some(scalar) = lower_integer_math_relation(&strict)
                        && let Ok(proof) = self.prove(&scalar)
                    {
                        let one = IntegerMathTerm::literal(IntegerValue::Unsigned(1));
                        let relation = node(
                            Proposition::IntegerMathLessOrEqual(
                                IntegerMathTerm::Add(smaller.clone(), Box::new(one.clone())),
                                (**larger).clone(),
                            ),
                            ProofRule::IntegerOrderDiscreteness {
                                relation: Box::new(proof),
                            },
                        );
                        let goal = Proposition::IntegerMathLessOrEqual(
                            IntegerMathTerm::Add(Box::new(left.clone()), Box::new(one)),
                            right.clone(),
                        );
                        if let Some(proof) =
                            self.translate_mathematical_order(relation, (**common).clone(), &goal)
                        {
                            return Some((proof, true));
                        }
                    }
                }
                if let Ok(proof) = self.prove(&Proposition::IntegerMathLessOrEqual(
                    (**smaller).clone(),
                    (**larger).clone(),
                )) && let Some(proof) = self.translate_mathematical_order(
                    proof,
                    (**common).clone(),
                    &Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
                ) {
                    return Some((proof, false));
                }
            }
        }
        None
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
