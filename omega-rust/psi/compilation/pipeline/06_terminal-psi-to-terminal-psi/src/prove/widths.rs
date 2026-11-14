/*
Range goals at a parameter width. A width-generic body is judged once, with
its width `N` a rigid `u64` input (`wiki/spec/language/numeric_values.md`,
"Width-indexed integers": the generic body must hold at every admitted
width). Its carrier's endpoints are terms over `N`, `2^N - 1` unsigned and
`-2^(N-1)`, `2^(N-1) - 1` signed (`IntegerMathTerm::carrier_maximum`), so
the literal bounds the rest of this prover carries say nothing about them.

A goal `sum <= endpoint` (or `endpoint <= sum` for a lower endpoint) is
proved by bounding each operand of the sum by a term, adding the bounds, and
comparing the total with the endpoint. An operand's bound is one of:

- its carrier's endpoint (`IntegerCarrierBound`);
- the literal its result equation fixes it to, weakened from the equation;
- the bound of the operand an exact conversion into the width carried it
  from (`IntegerCastBound`, mathematical form): how a literal lands there;
- a hypothesis ordering it against another value of the carrier, chained to
  that value's carrier endpoint; a strict order gains one by discreteness.

Bounds are added by translating each order by the other side's term
(`IntegerMathOrderTranslation`), joined where two terms differ by a constant
(`ClosedIntegerRelation`, which the kernel decides by ring normalization
with each power of two an opaque atom). The total meets the endpoint the
same way, or, when they differ by a power of two, through the width's own
lower bound: from `m <= N`, `2^m <= 2^N` (`IntegerPowerOfTwoOrder`), the
only fact about a power of two these goals need. From the declaration's
`N >= 1` that gives `2 <= 2^N`, so `1` inhabits `u<N>`; it gives only
`1 <= 2^(N-1)`, so `1` need not inhabit `i<N>`.

Wrapping, saturating and trapping arithmetic at a parameter width are
refused before they reach here (`typed_trees_to_terminal_psi`): they need
reduction modulo `2^N`, which no term states.
*/

use proof_admission::{PrimitiveJudgment, ProofRule, compare_integer_math_terms, decide_primitive};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};

use super::{Direction, ProofNode, Prover, Unproven, flip, node, primitive};

/// Whether `term` names a parameter width: a value of a parameter-width
/// carrier, or a power of two whose exponent is open.
pub(super) fn mentions_parameter_width(term: &IntegerMathTerm) -> bool {
    match term {
        IntegerMathTerm::MathValue { source_type, .. } => source_type.bits().is_none(),
        IntegerMathTerm::Embed(term) => term
            .scalar_type()
            .as_integer()
            .is_some_and(|integer| integer.bits().is_none()),
        IntegerMathTerm::ShiftLeft { count, .. } if !closed(count) => true,
        IntegerMathTerm::IntegerLiteral(_) => false,
        _ => term.operands().into_iter().any(mentions_parameter_width),
    }
}

/// Whether the goal `left <= right` has the form this module proves: one
/// side a value, a literal or the sum of two of them, the other an
/// endpoint, and a parameter width named somewhere.
pub(super) fn covered(left: &IntegerMathTerm, right: &IntegerMathTerm) -> bool {
    (mentions_parameter_width(left) || mentions_parameter_width(right))
        && (operands(left).is_some() || operands(right).is_some())
}

fn closed(term: &IntegerMathTerm) -> bool {
    match term {
        IntegerMathTerm::IntegerLiteral(_) => true,
        IntegerMathTerm::MathValue { .. } | IntegerMathTerm::Embed(_) => false,
        _ => term.operands().into_iter().all(closed),
    }
}

/// The operands of a sum the goal bounds: one leaf, or two added.
fn operands(term: &IntegerMathTerm) -> Option<Vec<&IntegerMathTerm>> {
    let leaf = |term: &IntegerMathTerm| {
        matches!(
            term,
            IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_)
        )
    };
    match term {
        IntegerMathTerm::Add(left, right) if leaf(left) && leaf(right) => {
            Some(vec![left.as_ref(), right.as_ref()])
        }
        term if leaf(term) => Some(vec![term]),
        _ => None,
    }
}

fn literal(value: i128) -> IntegerMathTerm {
    IntegerMathTerm::literal(IntegerValue::Signed(value))
}

fn add(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
    IntegerMathTerm::Add(Box::new(left), Box::new(right))
}

fn subtract(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
    IntegerMathTerm::Subtract(Box::new(left), Box::new(right))
}

fn order(low: IntegerMathTerm, high: IntegerMathTerm) -> Proposition {
    Proposition::IntegerMathLessOrEqual(low, high)
}

/// `low + addend <= high + addend` from `low <= high`.
fn translate(proof: ProofNode, addend: IntegerMathTerm) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(low, high) = &proof.conclusion else {
        return None;
    };
    let conclusion = order(add(low.clone(), addend.clone()), add(high.clone(), addend.clone()));
    Some(node(
        conclusion,
        ProofRule::IntegerMathOrderTranslation {
            relation: Box::new(proof),
            addend,
        },
    ))
}

/// The two ends of a proved mathematical `<=`.
fn ends(proof: &ProofNode) -> Option<(&IntegerMathTerm, &IntegerMathTerm)> {
    match &proof.conclusion {
        Proposition::IntegerMathLessOrEqual(low, high) => Some((low, high)),
        _ => None,
    }
}

/// The exponent of each open power of two in `term`.
fn exponents(term: &IntegerMathTerm, found: &mut Vec<IntegerMathTerm>) {
    match term {
        IntegerMathTerm::ShiftLeft { value, count }
            if !closed(count)
                && **value == IntegerMathTerm::literal(IntegerValue::Unsigned(1)) =>
        {
            if !found.contains(count) {
                found.push(count.as_ref().clone());
            }
        }
        _ => {
            for operand in term.operands() {
                exponents(operand, found);
            }
        }
    }
}

impl Prover<'_> {
    /// `left <= right` where a parameter width is named.
    pub(super) fn width_goal(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<ProofNode, Unproven> {
        let (sum, direction) = match (operands(left), operands(right)) {
            (Some(sum), _) => (sum, Direction::Upper),
            (None, Some(sum)) => (sum, Direction::Lower),
            (None, None) => return Err("a width goal over no sum of values"),
        };
        let endpoint = match direction {
            Direction::Upper => right,
            Direction::Lower => left,
        };
        let candidates = sum
            .iter()
            .map(|operand| self.operand_bounds(operand, direction))
            .collect::<Vec<_>>();
        let combinations: Vec<Vec<&ProofNode>> = match candidates.as_slice() {
            [only] => only.iter().map(|bound| vec![bound]).collect(),
            [first, second] => first
                .iter()
                .flat_map(|a| second.iter().map(move |b| vec![a, b]))
                .collect(),
            _ => Vec::new(),
        };
        for bounds in combinations {
            let Some(total) = self.total(&bounds, direction) else {
                continue;
            };
            let Some((low, high)) = ends(&total) else {
                continue;
            };
            let meet = match direction {
                Direction::Upper => self.width_order(high, endpoint),
                Direction::Lower => self.width_order(endpoint, low),
            };
            let Some(meet) = meet else {
                continue;
            };
            let proof = match direction {
                Direction::Upper => chain_math(total, meet),
                Direction::Lower => chain_math(meet, total),
            };
            if let Some(proof) = proof
                && proof.conclusion == order(left.clone(), right.clone())
            {
                return Ok(proof);
            }
        }
        Err("no bound of the operands meets the width's endpoint")
    }

    /// `operand <= bound` (`bound <= operand` for `Lower`) for each bound
    /// this module knows.
    fn operand_bounds(&mut self, operand: &IntegerMathTerm, direction: Direction) -> Vec<ProofNode> {
        let mut bounds = Vec::new();
        match operand {
            IntegerMathTerm::IntegerLiteral(_) => {
                bounds.extend(self.closed(order(operand.clone(), operand.clone())));
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                let subject = ScalarTerm::value(*value, ScalarType::Integer(*source_type));
                bounds.extend(self.value_bounds(&subject, *source_type, direction, 2));
            }
            _ => {}
        }
        bounds
    }

    fn value_bounds(
        &mut self,
        subject: &ScalarTerm,
        integer: IntegerType,
        direction: Direction,
        depth: usize,
    ) -> Vec<ProofNode> {
        let mut bounds = Vec::new();
        let ScalarTerm::Value { id, .. } = subject else {
            return bounds;
        };
        let value = IntegerMathTerm::MathValue {
            source_type: integer,
            value: *id,
        };
        bounds.extend(self.carrier_endpoint(&value, integer, direction));
        for index in 0..self.facts.len() {
            let proposition = self.facts[index].proposition.clone();
            let proof = self.facts[index].proof.clone();
            let axiom = self.facts[index].axiom;
            match &proposition {
                // The literal a result equation fixes the value to.
                Proposition::Equal(left, literal @ ScalarTerm::Integer { .. }) if left == subject => {
                    let oriented = match direction {
                        Direction::Upper => Some(proof),
                        Direction::Lower => flip(proof).ok(),
                    };
                    let Some(oriented) = oriented else { continue };
                    let (low, high) = match direction {
                        Direction::Upper => (value.clone(), lift(literal)),
                        Direction::Lower => (lift(literal), value.clone()),
                    };
                    bounds.push(node(
                        order(low, high),
                        ProofRule::IntegerOrderWeakening {
                            relation: Box::new(oriented),
                        },
                    ));
                }
                // An exact conversion keeps its operand's value.
                Proposition::Equal(
                    left,
                    ScalarTerm::IntegerExactCast {
                        source_type,
                        operand,
                        ..
                    },
                ) if left == subject && depth > 0 => {
                    let Some(axiom) = axiom else { continue };
                    for root in self.value_bounds(operand, *source_type, direction, depth - 1) {
                        let Some((low, high)) = ends(&root) else { continue };
                        let conclusion = match direction {
                            Direction::Upper => order(value.clone(), high.clone()),
                            Direction::Lower => order(low.clone(), value.clone()),
                        };
                        bounds.push(node(
                            conclusion,
                            ProofRule::IntegerCastBound {
                                root_bound: Box::new(root),
                                witness: proof_admission::IntegerCastChainWitness {
                                    root: operand.as_ref().clone(),
                                    target: subject.clone(),
                                    definition_axioms: vec![axiom],
                                },
                            },
                        ));
                    }
                }
                // An order against another value of the carrier, carried to
                // that value's own endpoint.
                Proposition::LessThan(low, high) | Proposition::LessOrEqual(low, high)
                    if low != high
                        && matches!(low, ScalarTerm::Value { .. })
                        && matches!(high, ScalarTerm::Value { .. })
                        && low.scalar_type() == high.scalar_type() =>
                {
                    let strict = matches!(proposition, Proposition::LessThan(..));
                    let other = match direction {
                        Direction::Upper if low == subject => high,
                        Direction::Lower if high == subject => low,
                        _ => continue,
                    };
                    let ScalarTerm::Value { id: other_id, .. } = other else {
                        continue;
                    };
                    let other_value = IntegerMathTerm::MathValue {
                        source_type: integer,
                        value: *other_id,
                    };
                    let Some(endpoint) = self.carrier_endpoint(&other_value, integer, direction)
                    else {
                        continue;
                    };
                    if let Some(bound) = self.relational_bound(
                        proof,
                        strict,
                        &value,
                        &other_value,
                        endpoint,
                        direction,
                    ) {
                        bounds.push(bound);
                    }
                }
                _ => {}
            }
        }
        bounds
    }

    /// `value <= endpoint - 1` from `value < other` and `other <= endpoint`
    /// (`value <= endpoint` from `value <= other`); the mirror for `Lower`.
    fn relational_bound(
        &mut self,
        relation: ProofNode,
        strict: bool,
        value: &IntegerMathTerm,
        other: &IntegerMathTerm,
        endpoint: ProofNode,
        direction: Direction,
    ) -> Option<ProofNode> {
        let one = literal(1);
        if !strict {
            let relation = self.mathematical(relation)?;
            return match direction {
                Direction::Upper => chain_math(relation, endpoint),
                Direction::Lower => chain_math(endpoint, relation),
            };
        }
        match direction {
            Direction::Upper => {
                // value + 1 <= other <= endpoint, then subtract the one.
                let stepped = node(
                    order(add(value.clone(), IntegerMathTerm::literal(IntegerValue::Unsigned(1))), other.clone()),
                    ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(relation),
                    },
                );
                let lowered = translate(chain_math(stepped, endpoint)?, literal(-1))?;
                let (low, _) = ends(&lowered)?;
                let rejoined = self.closed(order(value.clone(), low.clone()))?;
                chain_math(rejoined, lowered)
            }
            Direction::Lower => {
                // endpoint + 1 <= other + 1 <= value.
                let stepped = node(
                    order(add(other.clone(), IntegerMathTerm::literal(IntegerValue::Unsigned(1))), value.clone()),
                    ProofRule::IntegerOrderDiscreteness {
                        relation: Box::new(relation),
                    },
                );
                chain_math(translate(endpoint, one)?, stepped)
            }
        }
    }

    /// The carrier endpoint of `value` in `direction`, as a primitive.
    fn carrier_endpoint(
        &self,
        value: &IntegerMathTerm,
        integer: IntegerType,
        direction: Direction,
    ) -> Option<ProofNode> {
        let conclusion = match direction {
            Direction::Upper => order(value.clone(), IntegerMathTerm::carrier_maximum(integer)),
            Direction::Lower => order(IntegerMathTerm::carrier_minimum(integer), value.clone()),
        };
        decide_primitive(
            self.context,
            &conclusion,
            PrimitiveJudgment::IntegerCarrierBound,
        )
        .ok()?;
        Some(primitive(conclusion, PrimitiveJudgment::IntegerCarrierBound))
    }

    /// A scalar order restated over the mathematical integers: chained with
    /// the closed `right <= right`, whose conclusion the kernel matches in
    /// its mathematical reading.
    fn mathematical(&self, proof: ProofNode) -> Option<ProofNode> {
        let (left, right) = match &proof.conclusion {
            Proposition::IntegerMathLessOrEqual(..) => return Some(proof),
            Proposition::LessOrEqual(left, right) => (left.clone(), right.clone()),
            _ => return None,
        };
        let reflexive = self.closed(Proposition::LessOrEqual(right.clone(), right.clone()))?;
        Some(node(
            order(lift(&left), lift(&right)),
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(proof),
                middle_less_or_equal_right: Box::new(reflexive),
            },
        ))
    }

    /// The bounds of a sum's operands added: `a + b <= A + B` from
    /// `a <= A` and `b <= B` (mirrored for `Lower`).
    fn total(&mut self, bounds: &[&ProofNode], direction: Direction) -> Option<ProofNode> {
        match bounds {
            [only] => Some((*only).clone()),
            [first, second] => {
                // For `Upper`: a + b <= A + b, and b + A <= B + A.
                let first_outer = match direction {
                    Direction::Upper => ends(first)?.1.clone(),
                    Direction::Lower => ends(first)?.0.clone(),
                };
                let second_inner = match direction {
                    Direction::Upper => ends(second)?.0.clone(),
                    Direction::Lower => ends(second)?.1.clone(),
                };
                let moved_first = translate((*first).clone(), second_inner.clone())?;
                let moved_second = translate((*second).clone(), first_outer.clone())?;
                match direction {
                    Direction::Upper => {
                        let (_, middle) = ends(&moved_first)?;
                        let (start, _) = ends(&moved_second)?;
                        let joined = self.closed(order(middle.clone(), start.clone()))?;
                        chain_math(chain_math(moved_first, joined)?, moved_second)
                    }
                    Direction::Lower => {
                        let (_, end) = ends(&moved_second)?;
                        let (middle, _) = ends(&moved_first)?;
                        let joined = self.closed(order(end.clone(), middle.clone()))?;
                        chain_math(chain_math(moved_second, joined)?, moved_first)
                    }
                }
            }
            _ => None,
        }
    }

    /// `low <= high` for two terms that differ by a constant, or by a power
    /// of two the width's lower bound orders.
    fn width_order(&mut self, low: &IntegerMathTerm, high: &IntegerMathTerm) -> Option<ProofNode> {
        if matches!(compare_integer_math_terms(low, high), Ok(Some(ordering)) if ordering.is_le())
        {
            return self.closed(order(low.clone(), high.clone()));
        }
        let mut found = Vec::new();
        exponents(low, &mut found);
        exponents(high, &mut found);
        for exponent in found {
            let Some(power_order) = self.power_order(&exponent) else {
                continue;
            };
            let Some((least, power)) = ends(&power_order).map(|(l, p)| (l.clone(), p.clone()))
            else {
                continue;
            };
            // Cancel the power where `high` adds it, or where `low`
            // subtracts it.
            for addend in [
                subtract(high.clone(), power.clone()),
                subtract(low.clone(), least.clone()),
            ] {
                let Some(moved) = translate(power_order.clone(), addend) else {
                    continue;
                };
                let Some((start, end)) = ends(&moved).map(|(s, e)| (s.clone(), e.clone())) else {
                    continue;
                };
                let (Some(into), Some(out)) = (
                    self.closed(order(low.clone(), start)),
                    self.closed(order(end, high.clone())),
                ) else {
                    continue;
                };
                if let Some(proof) = chain_math(into, moved).and_then(|p| chain_math(p, out)) {
                    return Some(proof);
                }
            }
        }
        None
    }

    /// `2^m <= 2^exponent` for the largest `m` the exponent's lower bound
    /// gives: the exponent is a width `N` or `N - 1`.
    fn power_order(&mut self, exponent: &IntegerMathTerm) -> Option<ProofNode> {
        let (width, reduced) = match exponent {
            IntegerMathTerm::MathValue { .. } => (exponent, false),
            IntegerMathTerm::Subtract(width, one)
                if **one == IntegerMathTerm::literal(IntegerValue::Unsigned(1)) =>
            {
                (width.as_ref(), true)
            }
            _ => return None,
        };
        let IntegerMathTerm::MathValue { value, .. } = width else {
            return None;
        };
        let bound = self.bound(*value, Direction::Lower)?;
        let least = match bound.literal {
            IntegerValue::Unsigned(least) => i128::try_from(least).ok()?,
            IntegerValue::Signed(least) => least,
        };
        let at_width = self.mathematical(bound.proof)?;
        let premise = if reduced {
            if least < 1 {
                return None;
            }
            let moved = translate(at_width, literal(-1))?;
            let (start, end) = ends(&moved).map(|(s, e)| (s.clone(), e.clone()))?;
            let into = self.closed(order(literal(least - 1), start))?;
            let out = self.closed(order(end, exponent.clone()))?;
            chain_math(chain_math(into, moved)?, out)?
        } else {
            at_width
        };
        let (low, high) = ends(&premise).map(|(l, h)| (l.clone(), h.clone()))?;
        let power = |count| IntegerMathTerm::ShiftLeft {
            value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
            count: Box::new(count),
        };
        Some(node(
            order(power(low), power(high)),
            ProofRule::IntegerPowerOfTwoOrder {
                order: Box::new(premise),
            },
        ))
    }
}

/// A scalar operand's mathematical term.
fn lift(term: &ScalarTerm) -> IntegerMathTerm {
    IntegerMathTerm::embed(term.clone()).expect("an integer operand")
}

/// `a <= c` from `a <= b` and `b <= c`, both mathematical.
fn chain_math(first: ProofNode, second: ProofNode) -> Option<ProofNode> {
    let (low, _) = ends(&first)?;
    let (_, high) = ends(&second)?;
    let conclusion = order(low.clone(), high.clone());
    Some(node(
        conclusion,
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(first),
            middle_less_or_equal_right: Box::new(second),
        },
    ))
}
