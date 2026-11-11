/*
Range propagation through the operations that have no definition-bound rule
of their own: divide, remainder, the shifts, bitwise and, bitwise or.

`wiki/spec/proofs/contracts.md`, "Required automatic discharge": "Each integer
value defined by add, subtract, multiply, divide, remainder, shift or cast
receives bounds from its operands' bounds, once, in definition order, by the
arithmetic of `IntegerAffineBound`." The kernel replays one such operation as
a one-step endpoint chain: the witness roots at the operand that is a value,
cites the operation's result equation as the only definition, and lands the
other operand on the earlier equation that fixes it to a literal
(`wiki/spec/terminal-psi/integer_certificates.md`, "IntegerAffineBound"). The
prover supplies the root's own bound and reads the image the kernel computes
(`map_integer_affine_bound`); it does no endpoint arithmetic itself.

An unsigned remainder by a divisor that is not a literal has the kernel's
relational form instead: `result < divisor` from `0 < divisor`
(`remainder_below_divisor`). That is an order step between two values, and a
bound on the result through the divisor's bound.

An exact remainder by a divisor that is not a literal also takes a bound from
its dividend, for either sign: it lies between zero and the dividend, so
`dividend <= U` gives `result <= max(U, 0)` and `L <= dividend` gives
`min(L, 0) <= result`, once the divisor is proved positive or below minus one
(`remainder_within_dividend`). The kernel trusts that statement as a
provisional rule; its mathematical core does not derive it.
*/

use std::cmp::Ordering;

use proof_admission::{
    IntegerAffineWitness, IntegerCastChainWitness, PrimitiveJudgment, ProofRule,
    check_integer_affine_witness, check_integer_cast_bound_conversion,
    check_integer_cast_chain_witness, map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType,
};

use super::{Bound, Direction, ProofNode, Prover, Unproven, node};

/// The operand an endpoint chain can root at and the one it must land, for
/// each operation the chain replays with exactly one step. A shift's root is
/// its value and never its count.
fn chain_operands(expression: &ScalarTerm) -> Vec<(&ScalarTerm, &ScalarTerm)> {
    match expression {
        ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. } => vec![(left, right)],
        ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. } => {
            vec![(left, right), (right, left)]
        }
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => vec![(value, count)],
        _ => Vec::new(),
    }
}

impl Prover<'_> {
    /// The earlier axiom that fixes `term` to a literal, which a chain step
    /// cites as the landing of its literal operand.
    fn landing(&self, term: &ScalarTerm, before: usize) -> Option<usize> {
        self.axioms.get(..before)?.iter().position(|axiom| {
            matches!(
                axiom,
                Proposition::Equal(subject, literal)
                    if subject == term && literal.integer_value().is_some()
            )
        })
    }

    /// A bound on `subject`, the output of the operation `expression` that
    /// axiom `axiom` defines, through a one-step endpoint chain from the
    /// operand that is not a literal. Both of the root's bounds are tried,
    /// because a step may reverse the order (a negative divisor) or give the
    /// same image from either (a mask).
    pub(super) fn chain_step(
        &mut self,
        axiom: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        expression: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let mut best: Option<Bound> = None;
        for (root, sibling) in chain_operands(expression) {
            if !matches!(root, ScalarTerm::Value { .. }) {
                continue;
            }
            let landing = match sibling {
                ScalarTerm::Integer { .. } => None,
                _ => match self.landing(sibling, axiom) {
                    Some(landing) => Some(landing),
                    None => continue,
                },
            };
            let witness = IntegerAffineWitness {
                root: root.clone(),
                target: subject.clone(),
                definition_axioms: vec![axiom],
                literal_axioms: vec![landing],
            };
            let Ok(form) = check_integer_affine_witness(self.context, self.axioms, &witness) else {
                continue;
            };
            for root_direction in [Direction::Upper, Direction::Lower] {
                let Ok(root_bound) = self.evidence(root, root_direction) else {
                    continue;
                };
                let Ok(image) = map_integer_affine_bound(&form, &root_bound.conclusion) else {
                    continue;
                };
                let literal = match (&image, direction) {
                    (Proposition::LessOrEqual(left, right), Direction::Upper)
                        if left == subject =>
                    {
                        right.integer_value()
                    }
                    (Proposition::LessOrEqual(left, right), Direction::Lower)
                        if right == subject =>
                    {
                        left.integer_value()
                    }
                    _ => None,
                };
                let Some((_, literal)) = literal else {
                    continue;
                };
                let better = best.as_ref().is_none_or(|best| {
                    matches!(
                        (integer.compare(literal, best.literal), direction),
                        (Some(Ordering::Less), Direction::Upper)
                            | (Some(Ordering::Greater), Direction::Lower)
                    )
                });
                if better {
                    best = Some(Bound {
                        literal,
                        proof: node(
                            image,
                            ProofRule::IntegerAffineBound {
                                root_bound: Box::new(root_bound),
                                witness: witness.clone(),
                            },
                        ),
                    });
                }
            }
        }
        best
    }

    /// `output < divisor` for an unsigned remainder `output == dividend %
    /// divisor`, from `0 < divisor`: the kernel's relational remainder form,
    /// rooted at the divisor. `None` when the form does not apply (a signed
    /// carrier) or the divisor is not known positive.
    pub(super) fn remainder_below_divisor(
        &mut self,
        axiom: usize,
        output: &ScalarTerm,
        expression: &ScalarTerm,
    ) -> Option<(ScalarTerm, ProofNode)> {
        let ScalarTerm::ExactIntegerRemainder {
            scalar_type, right, ..
        } = expression
        else {
            return None;
        };
        let divisor = right.as_ref();
        if !matches!(divisor, ScalarTerm::Value { .. }) || divisor == output {
            return None;
        }
        let witness = IntegerAffineWitness {
            root: divisor.clone(),
            target: output.clone(),
            definition_axioms: vec![axiom],
            literal_axioms: vec![None],
        };
        let form = check_integer_affine_witness(self.context, self.axioms, &witness).ok()?;
        let positive = self.positive(*scalar_type, divisor)?;
        let image = map_integer_affine_bound(&form, &positive.conclusion).ok()?;
        matches!(&image, Proposition::LessThan(left, right) if left == output && right == divisor)
            .then(|| {
                (
                    divisor.clone(),
                    node(
                        image,
                        ProofRule::IntegerAffineBound {
                            root_bound: Box::new(positive),
                            witness,
                        },
                    ),
                )
            })
    }

    /// A bound on `output == dividend % divisor`, an exact remainder whose
    /// divisor is a value no literal fixes, from the dividend's bound: the
    /// kernel's remainder-within-dividend form, a provisional rule
    /// (`AcceptedProofRule::IntegerRemainderWithinDividend`). It needs the
    /// divisor proved positive or proved below minus one, and maps
    /// `dividend <= U` to `output <= max(U, 0)` and `L <= dividend` to
    /// `min(L, 0) <= output`. `None` when the kernel refuses the witness (a
    /// literal or landed divisor is `chain_step`'s) or either premise is
    /// unproved.
    pub(super) fn remainder_within_dividend(
        &mut self,
        axiom: usize,
        output: &ScalarTerm,
        expression: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } = expression
        else {
            return None;
        };
        let (dividend, divisor) = (left.as_ref(), right.as_ref());
        let witness = IntegerAffineWitness {
            root: dividend.clone(),
            target: output.clone(),
            definition_axioms: vec![axiom],
            literal_axioms: vec![None],
        };
        let form = check_integer_affine_witness(self.context, self.axioms, &witness).ok()?;
        let dividend_bound = self.evidence(dividend, direction).ok()?;
        let nonzero = match self.positive(*scalar_type, divisor) {
            Some(positive) => positive,
            None if scalar_type.sign() == IntegerSign::Signed => {
                let minus_one = ScalarTerm::integer(
                    *scalar_type,
                    semantic_vocabulary::IntegerValue::Signed(-1),
                )
                .ok()?;
                self.prove(&Proposition::LessThan(divisor.clone(), minus_one))
                    .ok()?
            }
            None => return None,
        };
        let parts = vec![dividend_bound, nonzero];
        let root =
            Proposition::Conjunction(parts.iter().map(|part| part.conclusion.clone()).collect());
        let image = map_integer_affine_bound(&form, &root).ok()?;
        let literal = match (&image, direction) {
            (Proposition::LessOrEqual(left, right), Direction::Upper) if left == output => {
                right.integer_value()
            }
            (Proposition::LessOrEqual(left, right), Direction::Lower) if right == output => {
                left.integer_value()
            }
            _ => None,
        }?
        .1;
        Some(Bound {
            literal,
            proof: node(
                image,
                ProofRule::IntegerAffineBound {
                    root_bound: Box::new(node(root, ProofRule::ConjunctionIntroduction(parts))),
                    witness,
                },
            ),
        })
    }

    /// The kernel's image of an exact left shift's operand bounds: a bound in
    /// `direction` on the mathematical term `value << count`, from the
    /// value's bound and what is known of the count (its literal when an
    /// equation fixes it, else its upper bound, with `0 <= count` for a
    /// signed count). The kernel picks the count endpoint by the sign of the
    /// value's bound (`map_direct_shift_left_bound`).
    pub(super) fn shift_left_image(
        &mut self,
        value: &IntegerMathTerm,
        count: &IntegerMathTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let leaf = |leaf: &IntegerMathTerm| match leaf {
            IntegerMathTerm::MathValue { source_type, value } => Ok((
                *source_type,
                ScalarTerm::value(*value, ScalarType::Integer(*source_type)),
            )),
            _ => Err("shift operand is not a value"),
        };
        let (value_type, value) = leaf(value)?;
        let (count_type, count) = leaf(count)?;
        let mut parts = vec![self.evidence(&value, direction)?];
        let fixed = self
            .facts
            .iter()
            .find(|fact| {
                matches!(
                    &fact.proposition,
                    Proposition::Equal(subject, literal)
                        if subject == &count && literal.integer_value().is_some()
                )
            })
            .map(|fact| fact.proof.clone());
        match fixed {
            Some(fixed) => parts.push(fixed),
            None => {
                parts.push(self.evidence(&count, Direction::Upper)?);
                if count_type.sign() == IntegerSign::Signed {
                    let zero = ScalarTerm::integer(count_type, count_type.zero())
                        .map_err(|_| "shift count zero")?;
                    parts.push(self.prove(&Proposition::LessOrEqual(zero, count.clone()))?);
                }
            }
        }
        let target = ScalarTerm::exact_integer_shift_left(
            value_type,
            count_type,
            value.clone(),
            count.clone(),
        )
        .map_err(|_| "shift term")?;
        let witness = IntegerAffineWitness {
            root: value,
            target,
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(self.context, self.axioms, &witness)
            .map_err(|_| "the kernel refuses the shift's witness")?;
        let root =
            Proposition::Conjunction(parts.iter().map(|part| part.conclusion.clone()).collect());
        let image = map_integer_affine_bound(&form, &root)
            .map_err(|_| "the kernel maps no bound through the shift")?;
        Ok(node(
            image,
            ProofRule::IntegerAffineBound {
                root_bound: Box::new(node(root, ProofRule::ConjunctionIntroduction(parts))),
                witness,
            },
        ))
    }

    /// A bound on `subject`, the output of the widening or exact cast
    /// `expression` that axiom `axiom` defines: the operand's bound with its
    /// literal restated in the output's carrier (`IntegerCastBound`; "A
    /// checked root-bound child maps the same mathematical endpoint into the
    /// final carrier"). An operand bound outside the output's carrier maps
    /// to nothing, which leaves the output its carrier bound.
    pub(super) fn cast_bound(
        &mut self,
        axiom: usize,
        subject: &ScalarTerm,
        integer: IntegerType,
        expression: &ScalarTerm,
        direction: Direction,
    ) -> Option<Bound> {
        let (ScalarTerm::IntegerWiden {
            source_type,
            operand,
            ..
        }
        | ScalarTerm::IntegerExactCast {
            source_type,
            operand,
            ..
        }) = expression
        else {
            return None;
        };
        let ScalarTerm::Value { id, .. } = operand.as_ref() else {
            return None;
        };
        let root = self.bound(*id, direction)?;
        let literal = source_type.exact_cast_value_to(integer, root.literal)?;
        let conclusion = Self::oriented(
            subject,
            ScalarTerm::integer(integer, literal).ok()?,
            direction,
        );
        let witness = IntegerCastChainWitness {
            root: operand.as_ref().clone(),
            target: subject.clone(),
            definition_axioms: vec![axiom],
        };
        let chain = check_integer_cast_chain_witness(self.context, self.axioms, &witness).ok()?;
        check_integer_cast_bound_conversion(&chain, &root.proof.conclusion, &conclusion).ok()?;
        Some(Bound {
            literal,
            proof: node(
                conclusion,
                ProofRule::IntegerCastBound {
                    root_bound: Box::new(root.proof),
                    witness,
                },
            ),
        })
    }

    /// `value <= limit` (or `limit <= value`) over mathematical integers,
    /// the representability goal of an exact cast: the value's bound in its
    /// own carrier, restated over its mathematical value, then the closed
    /// relation between that bound and the limit.
    pub(super) fn value_in_carrier(
        &mut self,
        source_type: IntegerType,
        value: semantic_vocabulary::ValueId,
        term: &IntegerMathTerm,
        limit: &IntegerMathTerm,
        direction: Direction,
    ) -> Result<ProofNode, Unproven> {
        let subject = ScalarTerm::value(value, ScalarType::Integer(source_type));
        let bound = self.bound(value, direction).ok_or("value has no bound")?;
        let literal = IntegerMathTerm::literal(bound.literal);
        let (restated, endpoint) = match direction {
            Direction::Upper => (
                Proposition::IntegerMathLessOrEqual(term.clone(), literal),
                0,
            ),
            Direction::Lower => (
                Proposition::IntegerMathLessOrEqual(literal, term.clone()),
                1,
            ),
        };
        // Substituting the value for itself changes nothing; the rule lets
        // a carrier order conclude over the mathematical value.
        let unchanged = super::primitive(
            Proposition::Equal(subject.clone(), subject),
            PrimitiveJudgment::ReflexiveEquality,
        );
        let mathematical = node(
            restated,
            ProofRule::IntegerOrderSubstitution {
                relation: Box::new(bound.proof),
                equality: Box::new(unchanged),
                endpoint,
            },
        );
        self.land(mathematical, term, limit, direction)
    }
}
