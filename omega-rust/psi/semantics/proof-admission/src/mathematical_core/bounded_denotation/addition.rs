//! Shared integer addition and checked correlated bound derivations.
//!
//! Addition monotonicity and subtraction cancellation are fixed assumptions,
//! not per-instance conclusions. The original affine witness checker runs
//! first. A conclusion whose endpoints both evaluate inside `fixed_magnitude`
//! is `IntLe` between two canonical numerals: the binary numeral laws decide
//! a true relation outright, and a false one is emptied through a checked
//! false premise. Sums still open keep the monotonicity and cancellation
//! chain; once the correlated difference has collapsed to its numeral `n`
//! the applicative `add (sub e r) r` cancellation cannot match `add n r`,
//! so the chain substitutes the checked numeral-operation equation
//! `add n r = e` — an exact interned assumption — in its place.
//!
//! The exact-add definition bound cites a semantic `out = l + r` equation
//! beside the two operand bounds: two-sided monotonicity combines the
//! operand evidence into a bound on `add l r`, the cited definition
//! transports that bound onto `out`, and a checked `add lb rb = k`
//! numeral equation lands the conclusion's literal endpoint. The direct
//! `IntegerAffineBound` form skips the citation: its premise is the
//! conjunction of the two operand bounds and its conclusion names the
//! `add` application itself, so the same monotone chain lands the bound
//! on `add l r` without an output transport. Each operand's evidence
//! re-shapes to the required direction — an oriented `≤` stands, an
//! `Equal` transports through `eq_le`, a literal addend is its own
//! endpoint through `refl` — while `Truth` over an open addend cites an
//! interned carrier-membership bound for its operand — and a conclusion
//! outside the fixed literal range keeps the explicit instance fallback.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType,
};

use super::super::scheme_dsl::{self, apps, id, pi, scheme_at, v};
use super::binary_numerals;
use super::{BoundedDenotationError, Declaration, Denotation, IntegerLaw, Term, TermHandle};
use crate::{ClosedIntegerEvaluator, kernel::KernelError, proof::ProofError};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    Monotone,
    CancelSubtract,
    /// `a ≤ b → c ≤ d → add a c ≤ add b d` — two-sided monotonicity.
    MonotoneBoth,
}

#[derive(Default)]
pub(super) struct Addition {
    operation: Option<u32>,
    laws: BTreeMap<Law, u32>,
    /// `add l r = s` equations — numeral-operation bridges between an
    /// `add` application over evaluated operands and their denoted sum.
    /// Each is interned once by its checked `(l, r, s)` value triple.
    pub(super) numeral_sums: BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
}

impl Denotation {
    pub(super) fn add_operation(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.addition.operation {
            return Ok(position);
        }
        let integer = self.integer_constant()?;
        let result = self.arena.insert(Term::Pi {
            domain: integer,
            codomain: integer,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: integer,
            codomain: result,
        });
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.addition.operation = Some(position);
        Ok(position)
    }

    pub(super) fn add_terms(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.add_operation()?;
        let operation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: operation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    fn add_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(&position) = self.addition.laws.get(&law) {
            position
        } else {
            let integer = self.integer()?;
            let operation = self.add_operation()?;
            let constant = |position| scheme_at(position, Vec::new());
            let carrier = || constant(integer);
            let add = |left, right| apps(constant(operation), [left, right]);
            let statement = match law {
                Law::Monotone => {
                    let relation = self.integer_less_or_equal()?;
                    let le = |left, right| apps(constant(relation), [left, right]);
                    pi(
                        "a",
                        carrier(),
                        pi(
                            "b",
                            carrier(),
                            pi(
                                "c",
                                carrier(),
                                pi(
                                    "_",
                                    le(v("a"), v("b")),
                                    le(add(v("a"), v("c")), add(v("b"), v("c"))),
                                ),
                            ),
                        ),
                    )
                }
                Law::MonotoneBoth => {
                    let relation = self.integer_less_or_equal()?;
                    let le = |left, right| apps(constant(relation), [left, right]);
                    pi(
                        "a",
                        carrier(),
                        pi(
                            "b",
                            carrier(),
                            pi(
                                "c",
                                carrier(),
                                pi(
                                    "d",
                                    carrier(),
                                    pi(
                                        "_",
                                        le(v("a"), v("b")),
                                        pi(
                                            "_",
                                            le(v("c"), v("d")),
                                            le(add(v("a"), v("c")), add(v("b"), v("d"))),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    )
                }
                Law::CancelSubtract => {
                    let subtract = self.subtract_operation()?;
                    pi(
                        "b",
                        carrier(),
                        pi(
                            "c",
                            carrier(),
                            id(
                                carrier(),
                                add(apps(constant(subtract), [v("b"), v("c")]), v("c")),
                                v("b"),
                            ),
                        ),
                    )
                }
            };
            let ty = scheme_dsl::build(&mut self.arena, &mut Vec::new(), &statement);
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.addition.laws.insert(law, position);
            position
        };
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    /// `Id Int (add l r) s` — the numeral-operation equation bridging an
    /// `add` application over evaluated operands to their denoted sum.
    /// Interned once per checked `(l, r, s)` value triple: the signature
    /// then names that exact equation in place of a whole-rule
    /// implication. `None` when the values do not satisfy `l + r = s` —
    /// a defensive refusal, since the caller derives them from a checked
    /// exact subtraction — so a miss keeps the instance fallback rather
    /// than naming a false equation.
    pub(super) fn numeral_sum(
        &mut self,
        addend: IntegerValue,
        augend: IntegerValue,
        sum: IntegerValue,
        addend_term: TermHandle,
        augend_term: TermHandle,
        sum_term: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let as_integer = |value: IntegerValue| match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        if as_integer(addend).add(&as_integer(augend)) != as_integer(sum) {
            return Ok(None);
        }
        if let Some(&position) = self.addition.numeral_sums.get(&(addend, augend, sum)) {
            return Ok(Some(self.constant(position)));
        }
        let integer = self.integer_constant()?;
        let application = self.add_terms(addend_term, augend_term)?;
        let ty = self.arena.insert(Term::Id {
            ty: integer,
            left: application,
            right: sum_term,
        });
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.addition
            .numeral_sums
            .insert((addend, augend, sum), position);
        Ok(Some(self.constant(position)))
    }

    /// Select a checked carrier-endpoint-minus-addend witness. The
    /// caller has already run affine_bound_relation, including citation checks.
    /// Integer equality normalization may reorder authored `Equal` endpoints;
    /// each transport therefore orients the denoted Id, not the source pair.
    /// Lower and upper bounds use the same monotonicity and cancellation laws:
    /// only the inequality endpoint being transported changes. Select that
    /// endpoint from the checked conclusion and require its exact carrier
    /// minimum/maximum, so a convenient subtraction cannot replace the witness.
    pub(super) fn correlated_add_bound_evidence(
        &mut self,
        premise: &Proposition,
        mut evidence: TermHandle,
        witness: &crate::IntegerAffineWitness,
        conclusion: &Proposition,
        definitions: &[(Proposition, TermHandle)],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } = &witness.target
        else {
            return Ok(None);
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed {
            return Ok(None);
        }
        let (lower, bound, sum) = match conclusion {
            Proposition::IntegerMathLessOrEqual(bound, sum @ IntegerMathTerm::Add(..)) => {
                (true, bound, sum)
            }
            Proposition::IntegerMathLessOrEqual(sum @ IntegerMathTerm::Add(..), bound) => {
                (false, bound, sum)
            }
            _ => return Ok(None),
        };
        // The checked conclusion's bound side is the carrier endpoint
        // literal, so it always evaluates. When the sum side still has a
        // canonical numeral the goal is a decidable `IntLe` between
        // numerals, discharged by the numeral laws rather than an instance
        // axiom; an open sum keeps the law chain below, substituting a
        // checked numeral-operation equation for the cancellation step
        // once the difference has collapsed to its numeral.
        let evaluate = |term: &IntegerMathTerm| {
            ClosedIntegerEvaluator::default()
                .evaluate_closed(term)
                .map_err(|error| {
                    BoundedDenotationError::Certificate(ProofError::PrimitiveJudgment(
                        KernelError::ClosedIntegerEvaluation(error),
                    ))
                })
        };
        let bound_value = evaluate(bound)?;
        let sum_value = evaluate(sum)?;
        if let (Some(bound_value), Some(sum_value)) = (&bound_value, &sum_value) {
            let (goal_left, goal_right, left_value, right_value) = if lower {
                (bound, sum, bound_value, sum_value)
            } else {
                (sum, bound, sum_value, bound_value)
            };
            return self.closed_order_bound_evidence(
                premise,
                evidence,
                conclusion,
                goal_left,
                goal_right,
                left_value,
                right_value,
            );
        }
        if sum_value.is_some() {
            return Ok(None);
        }
        let equality_for = |subject: &ScalarTerm| {
            definitions.iter().find_map(|(proposition, proof)| {
                let Proposition::Equal(first, second) = proposition else {
                    return None;
                };
                if first == subject {
                    Some((second, *proof, proposition))
                } else if second == subject {
                    Some((first, *proof, proposition))
                } else {
                    None
                }
            })
        };
        let (difference, equality) =
            if witness.definition_axioms.is_empty() && witness.literal_axioms.is_empty() {
                (&witness.root, None)
            } else if witness.definition_axioms.len() == 1 {
                let Some((difference, proof, proposition)) = equality_for(&witness.root) else {
                    return Ok(None);
                };
                (difference, Some((proof, proposition)))
            } else {
                return Ok(None);
            };
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type: subtract_type,
            left: endpoint,
            right: decrement,
        } = difference
        else {
            return Ok(None);
        };
        // A closed difference denotes to its numeral `n` rather than the
        // `subtract` application, so the applicative cancellation equation
        // `add (sub e r) r = e` cannot match `add n r`: that chain instead
        // substitutes the checked numeral-operation equation `add n r = e`
        // — an exact interned assumption — for the cancellation law.
        let closed_difference = difference.integer_value().is_some();
        let expected_premise = if lower {
            Proposition::LessOrEqual(witness.root.clone(), left.as_ref().clone())
        } else {
            Proposition::LessOrEqual(left.as_ref().clone(), witness.root.clone())
        };
        if subtract_type != scalar_type || decrement != right || premise != &expected_premise {
            return Ok(None);
        }
        let endpoint_value = if lower {
            scalar_type.minimum_value()
        } else {
            scalar_type.maximum_value()
        };
        let endpoint_literal = ScalarTerm::integer(*scalar_type, endpoint_value)
            .expect("carrier endpoint is representable");
        let endpoint_equality = if endpoint.integer_value() == Some((*scalar_type, endpoint_value))
        {
            None
        } else {
            let Some((literal, proof, proposition)) = equality_for(endpoint) else {
                return Ok(None);
            };
            if literal != &endpoint_literal {
                return Ok(None);
            }
            Some((proof, proposition))
        };
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let endpoint_term = self.fixed_scalar_term(endpoint)?;
        let difference_term = self.fixed_scalar_term(difference)?;
        if let Some((equality, proposition)) = equality {
            let root_term = self.fixed_scalar_term(&witness.root)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) =
                self.directed_equality(from, to, root_term, difference_term, equality)
            else {
                return Ok(None);
            };
            evidence = if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[root_term, difference_term, left_term, equality, evidence],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left_term, root_term, difference_term, equality, evidence],
                )?
            };
        }
        let sum = self.add_terms(left_term, right_term)?;
        let cancelled_sum = self.add_terms(difference_term, right_term)?;
        let ordered_terms = if lower {
            [difference_term, left_term, right_term, evidence]
        } else {
            [left_term, difference_term, right_term, evidence]
        };
        let order = self.add_law_application(Law::Monotone, &ordered_terms)?;
        let equality = if closed_difference {
            let (
                Some((_, difference_value)),
                Some((_, decrement_value)),
                Some((_, endpoint_value)),
            ) = (
                difference.integer_value(),
                decrement.integer_value(),
                endpoint.integer_value(),
            )
            else {
                return Ok(None);
            };
            let Some(equality) = self.numeral_sum(
                difference_value,
                decrement_value,
                endpoint_value,
                difference_term,
                right_term,
                endpoint_term,
            )?
            else {
                return Ok(None);
            };
            equality
        } else {
            self.add_law_application(Law::CancelSubtract, &[endpoint_term, right_term])?
        };
        let order = if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[cancelled_sum, endpoint_term, sum, equality, order],
            )?
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[sum, cancelled_sum, endpoint_term, equality, order],
            )?
        };
        if let Some((equality, proposition)) = endpoint_equality {
            let literal = self.fixed_scalar_term(&endpoint_literal)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) = self.directed_equality(from, to, endpoint_term, literal, equality)
            else {
                return Ok(None);
            };
            if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[endpoint_term, literal, sum, equality, order],
                )
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[sum, endpoint_term, literal, equality, order],
                )
            }
            .map(Some)
        } else {
            Ok(Some(order))
        }
    }

    /// Denote the checked direct-add bound: a `Conjunction` of the two
    /// operand bounds proves `k ≤ add l r` or `add l r ≤ k`, where `k` is
    /// the checked sum of the two endpoint literals. This is the
    /// `IntegerAffineBound` direct form — the witness names the `add`
    /// target itself with no endpoint steps or cited axioms — so the
    /// bound lands on the `add` application directly rather than
    /// transporting through an output definition. The operand evidence
    /// re-shapes exactly as the exact-add definition bound's does —
    /// oriented `≤` stands, `Equal` transports through `eq_le`, a literal
    /// addend is its own endpoint through `refl`, `Truth` over an open
    /// addend cites its interned carrier-membership bound — and the
    /// checked `add lb rb = k` numeral equation lands the conclusion's
    /// literal endpoint. A literal outside the representable numeral
    /// range keeps the explicit instance fallback.
    pub(super) fn direct_add_bound_evidence(
        &mut self,
        premise: &Proposition,
        evidence: TermHandle,
        witness: &crate::IntegerAffineWitness,
        conclusion: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::Conjunction(bounds) = premise else {
            return Ok(None);
        };
        let [left_bound, right_bound] = bounds.as_slice() else {
            return Ok(None);
        };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } = &witness.target
        else {
            return Ok(None);
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed
            || witness.root != **left
            || !witness.definition_axioms.is_empty()
            || !witness.literal_axioms.is_empty()
        {
            return Ok(None);
        }
        let (lower, bound, sum) = match conclusion {
            Proposition::IntegerMathLessOrEqual(bound, sum @ IntegerMathTerm::Add(..)) => {
                (true, bound, sum)
            }
            Proposition::IntegerMathLessOrEqual(sum @ IntegerMathTerm::Add(..), bound) => {
                (false, bound, sum)
            }
            _ => return Ok(None),
        };
        // The mapped bound is a canonical math literal; when its
        // magnitude leaves the representable numeral range the denoted
        // endpoint is an opaque `Int` constant no checked
        // `add lb rb = k` equation can name — keep the instance fallback.
        let IntegerMathTerm::IntegerLiteral(bound_literal) = bound else {
            return Ok(None);
        };
        let bound_value = if bound_literal.negative() {
            if bound_literal.magnitude() == (i128::MAX as u128) + 1 {
                IntegerValue::Signed(i128::MIN)
            } else {
                let Ok(magnitude) = i128::try_from(bound_literal.magnitude()) else {
                    return Ok(None);
                };
                IntegerValue::Signed(-magnitude)
            }
        } else {
            IntegerValue::Unsigned(bound_literal.magnitude())
        };
        // The conjunction denotes to a `Σ` pair: the first conjunct is
        // `fst`, the second — last — conjunct is `snd`.
        let left_evidence = self.arena.insert(Term::Fst { pair: evidence });
        let right_evidence = self.arena.insert(Term::Snd { pair: evidence });
        let Some((left_endpoint, left_order)) =
            self.add_bound_endpoint(left, left_bound, left_evidence, lower, scalar_type)?
        else {
            return Ok(None);
        };
        let Some((right_endpoint, right_order)) =
            self.add_bound_endpoint(right, right_bound, right_evidence, lower, scalar_type)?
        else {
            return Ok(None);
        };
        let left_endpoint_term = self.fixed_scalar_term(&left_endpoint)?;
        let right_endpoint_term = self.fixed_scalar_term(&right_endpoint)?;
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let bound_sum = self.add_terms(left_endpoint_term, right_endpoint_term)?;
        let operand_sum = self.add_terms(left_term, right_term)?;
        // The conclusion's `Add` math term must denote the same
        // application as the target's operands — the shared relation
        // already equated them, so this is the defensive shape check.
        let sum_term = self.math_term(sum)?;
        if !self.arena.structurally_equal(sum_term, operand_sum) {
            return Ok(None);
        }
        // `add lb rb ≤ add l r` (lower) or `add l r ≤ add lb rb` (upper).
        let order = if lower {
            self.add_law_application(
                Law::MonotoneBoth,
                &[
                    left_endpoint_term,
                    left_term,
                    right_endpoint_term,
                    right_term,
                    left_order,
                    right_order,
                ],
            )?
        } else {
            self.add_law_application(
                Law::MonotoneBoth,
                &[
                    left_term,
                    left_endpoint_term,
                    right_term,
                    right_endpoint_term,
                    left_order,
                    right_order,
                ],
            )?
        };
        let (Some((_, left_value)), Some((_, right_value))) = (
            left_endpoint.integer_value(),
            right_endpoint.integer_value(),
        ) else {
            return Ok(None);
        };
        let bound_term = self.math_term(bound)?;
        let Some(equality) = self.numeral_sum(
            left_value,
            right_value,
            bound_value,
            left_endpoint_term,
            right_endpoint_term,
            bound_term,
        )?
        else {
            return Ok(None);
        };
        if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[bound_sum, bound_term, operand_sum, equality, order],
            )
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[operand_sum, bound_sum, bound_term, equality, order],
            )
        }
        .map(Some)
    }

    /// Denote the checked exact-add definition bound: `lb ∧ rb` over the
    /// add's two operands plus the cited `output = l + r` definition prove
    /// `k ≤ output` or `output ≤ k`, where `k` is the checked sum of the
    /// two endpoint literals. Two-sided monotonicity combines the operand
    /// bounds; the cited definition equality — bridged by the checked
    /// `add l r = n` numeral equation when the definition's sum is closed —
    /// moves the bound onto `output`, and a second interned equation
    /// `add lb rb = k` lands the literal. Each operand's endpoint re-shapes
    /// its own evidence: an oriented `≤` stands, an `Equal` transports
    /// through `eq_le`, a literal operand uses `refl`, and `Truth` over an
    /// open operand cites its interned carrier-membership bound.
    pub(super) fn exact_add_definition_bound_evidence(
        &mut self,
        left_bound: &Proposition,
        left_evidence: TermHandle,
        right_bound: &Proposition,
        right_evidence: TermHandle,
        definition: &Proposition,
        definition_evidence: TermHandle,
        conclusion: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Proposition::LessOrEqual(conclusion_left, conclusion_right) = conclusion else {
            return Ok(None);
        };
        let (literal, output, lower) = if conclusion_left.integer_value().is_some() {
            (conclusion_left, conclusion_right, true)
        } else if conclusion_right.integer_value().is_some() {
            (conclusion_right, conclusion_left, false)
        } else {
            return Ok(None);
        };
        let (defined_output, expression) = match definition {
            Proposition::Equal(first, second) => match (first, second) {
                (output @ ScalarTerm::Value { .. }, ScalarTerm::ExactIntegerAdd { .. }) => {
                    (output, second)
                }
                (ScalarTerm::ExactIntegerAdd { .. }, output @ ScalarTerm::Value { .. }) => {
                    (output, first)
                }
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } = expression
        else {
            return Ok(None);
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed
            || defined_output != output
            || output.scalar_type() != ScalarType::Integer(*scalar_type)
        {
            return Ok(None);
        }
        let Some((left_endpoint, left_order)) =
            self.add_bound_endpoint(left, left_bound, left_evidence, lower, scalar_type)?
        else {
            return Ok(None);
        };
        let Some((right_endpoint, right_order)) =
            self.add_bound_endpoint(right, right_bound, right_evidence, lower, scalar_type)?
        else {
            return Ok(None);
        };
        let left_endpoint_term = self.fixed_scalar_term(&left_endpoint)?;
        let right_endpoint_term = self.fixed_scalar_term(&right_endpoint)?;
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let bound_sum = self.add_terms(left_endpoint_term, right_endpoint_term)?;
        let operand_sum = self.add_terms(left_term, right_term)?;
        // `add lb rb ≤ add l r` (lower) or `add l r ≤ add lb rb` (upper).
        let order = if lower {
            self.add_law_application(
                Law::MonotoneBoth,
                &[
                    left_endpoint_term,
                    left_term,
                    right_endpoint_term,
                    right_term,
                    left_order,
                    right_order,
                ],
            )?
        } else {
            self.add_law_application(
                Law::MonotoneBoth,
                &[
                    left_term,
                    left_endpoint_term,
                    right_term,
                    right_endpoint_term,
                    left_order,
                    right_order,
                ],
            )?
        };
        // `Id Int (add l' r') output'` — directly the cited definition when
        // the expression stays applicative, or the numeral-operation
        // equation `add l r = n` composed with the definition when the
        // closed sum denotes to its numeral `n`.
        let output_term = self.fixed_scalar_term(output)?;
        let expression_term = self.fixed_scalar_term(expression)?;
        let denoted = self.denote(definition)?;
        let Some((_, from, to)) = self.identity_parts(denoted) else {
            return Ok(None);
        };
        let sum_equality = if self.arena.structurally_equal(expression_term, operand_sum) {
            let Some(equality) =
                self.directed_equality(from, to, operand_sum, output_term, definition_evidence)
            else {
                return Ok(None);
            };
            equality
        } else {
            let (Some((_, left_value)), Some((_, right_value)), Some((_, sum_value))) = (
                left.integer_value(),
                right.integer_value(),
                expression.integer_value(),
            ) else {
                return Ok(None);
            };
            let Some(bridge) = self.numeral_sum(
                left_value,
                right_value,
                sum_value,
                left_term,
                right_term,
                expression_term,
            )?
            else {
                return Ok(None);
            };
            let Some(forward) =
                self.directed_equality(from, to, expression_term, output_term, definition_evidence)
            else {
                return Ok(None);
            };
            let integer = self.integer_constant()?;
            self.transitivity(
                integer,
                operand_sum,
                expression_term,
                output_term,
                bridge,
                forward,
            )
        };
        // `add lb' rb' ≤ output'` (lower) or `output' ≤ add lb' rb'` (upper).
        let order = if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[bound_sum, operand_sum, output_term, sum_equality, order],
            )?
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[operand_sum, output_term, bound_sum, sum_equality, order],
            )?
        };
        // The bound side is an `add` over two endpoint numerals; the
        // checked equation `add lb rb = k` lands the conclusion's literal.
        let (Some((_, left_value)), Some((_, right_value)), Some((_, bound_value))) = (
            left_endpoint.integer_value(),
            right_endpoint.integer_value(),
            literal.integer_value(),
        ) else {
            return Ok(None);
        };
        let literal_term = self.fixed_scalar_term(literal)?;
        let Some(equality) = self.numeral_sum(
            left_value,
            right_value,
            bound_value,
            left_endpoint_term,
            right_endpoint_term,
            literal_term,
        )?
        else {
            return Ok(None);
        };
        if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteLeft,
                &[bound_sum, literal_term, output_term, equality, order],
            )
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[output_term, bound_sum, literal_term, equality, order],
            )
        }
        .map(Some)
    }

    /// `IntLe min' operand'` (lower) or `IntLe operand' max'` (upper) —
    /// the carrier-membership bound a `Truth` premise over an open
    /// operand contributes. The named assumption records the exact
    /// fixed-carrier fact, interned once per `(operand, direction)`;
    /// quantifying it as a law would range over every `Int`, which the
    /// membership fact does not survive.
    fn carrier_bound(
        &mut self,
        operand: &ScalarTerm,
        lower: bool,
        integer_type: &IntegerType,
    ) -> Result<(ScalarTerm, TermHandle), BoundedDenotationError> {
        let value = if lower {
            integer_type.minimum_value()
        } else {
            integer_type.maximum_value()
        };
        let endpoint =
            ScalarTerm::integer(*integer_type, value).expect("carrier endpoint is representable");
        if let Some(&position) = self.carrier_bounds.get(&(operand.clone(), lower)) {
            return Ok((endpoint, self.constant(position)));
        }
        let proposition = if lower {
            Proposition::LessOrEqual(endpoint.clone(), operand.clone())
        } else {
            Proposition::LessOrEqual(operand.clone(), endpoint.clone())
        };
        let ty = self.denote(&proposition)?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.carrier_bounds
            .insert((operand.clone(), lower), position);
        Ok((endpoint, self.constant(position)))
    }

    /// One addend's bound endpoint: the checked literal it is bounded by,
    /// with the evidence re-shaped to `IntLe endpoint' operand'` (lower)
    /// or `IntLe operand' endpoint'` (upper). A literal operand bounded by
    /// `Truth` is its own endpoint through `refl`; `Truth` over an open
    /// operand cites its interned carrier-membership bound; `Equal`
    /// evidence transports through `eq_le`; an oriented `≤` stands in its
    /// own direction and refuses the other.
    fn add_bound_endpoint(
        &mut self,
        operand: &ScalarTerm,
        proposition: &Proposition,
        evidence: TermHandle,
        lower: bool,
        integer_type: &IntegerType,
    ) -> Result<Option<(ScalarTerm, TermHandle)>, BoundedDenotationError> {
        if operand.integer_value().is_some() {
            if proposition != &Proposition::Truth {
                return Ok(None);
            }
            let term = self.fixed_scalar_term(operand)?;
            let integer = self.integer_constant()?;
            let reflexive = self.arena.insert(Term::Refl {
                ty: integer,
                value: term,
            });
            let equality = self.integer_law_application(
                IntegerLaw::EqualityToLessOrEqual,
                &[term, term, reflexive],
            )?;
            return Ok(Some((operand.clone(), equality)));
        }
        match proposition {
            // The membership fact names the operand's own carrier; the
            // checked leaf already equates it with the add's type, and
            // the guard keeps a stray off-carrier operand a fallback.
            Proposition::Truth if operand.scalar_type() == ScalarType::Integer(*integer_type) => {
                self.carrier_bound(operand, lower, integer_type).map(Some)
            }
            Proposition::Truth => Ok(None),
            Proposition::Equal(first, second) => {
                let literal = if first == operand {
                    second
                } else if second == operand {
                    first
                } else {
                    return Ok(None);
                };
                let Some((actual, _)) = literal.integer_value() else {
                    return Ok(None);
                };
                if actual != *integer_type {
                    return Ok(None);
                }
                let operand_term = self.fixed_scalar_term(operand)?;
                let literal_term = self.fixed_scalar_term(literal)?;
                let denoted = self.denote(proposition)?;
                let Some((_, denoted_from, denoted_to)) = self.identity_parts(denoted) else {
                    return Ok(None);
                };
                let (want_from, want_to) = if lower {
                    (literal_term, operand_term)
                } else {
                    (operand_term, literal_term)
                };
                let Some(equality) =
                    self.directed_equality(denoted_from, denoted_to, want_from, want_to, evidence)
                else {
                    return Ok(None);
                };
                // `Id a b → IntLe a b` — the denoted endpoints already
                // match the requested orientation.
                self.integer_law_application(
                    IntegerLaw::EqualityToLessOrEqual,
                    &[want_from, want_to, equality],
                )
                .map(|order| Some((literal.clone(), order)))
            }
            Proposition::LessOrEqual(first, second) => {
                if first == second {
                    return Ok(None);
                }
                let bound = if second == operand && lower {
                    first
                } else if first == operand && !lower {
                    second
                } else {
                    return Ok(None);
                };
                let Some((actual, _)) = bound.integer_value() else {
                    return Ok(None);
                };
                if actual != *integer_type {
                    return Ok(None);
                }
                Ok(Some((bound.clone(), evidence)))
            }
            _ => Ok(None),
        }
    }

    /// The goal is `IntLe` between two canonical constants. A strict
    /// relation between fixed-width numerals is decided by the binary
    /// numeral laws; equal endpoints are `refl`; a false one is discharged
    /// by empty elimination through the checked premise when it is itself
    /// a false closed inequality. Strict endpoints outside `IntegerValue`'s
    /// range — or a premise without a closed contradiction — keep the
    /// caller's instance fallback.
    pub(super) fn closed_order_bound_evidence(
        &mut self,
        premise: &Proposition,
        evidence: TermHandle,
        conclusion: &Proposition,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
        left_value: &BigInt,
        right_value: &BigInt,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let numeral = |value: &BigInt| {
            let (negative, magnitude) = binary_numerals::fixed_magnitude(value)?;
            if !negative {
                Some(IntegerValue::Unsigned(magnitude))
            } else if magnitude == 1_u128 << 127 {
                Some(IntegerValue::Signed(i128::MIN))
            } else {
                i128::try_from(magnitude)
                    .ok()
                    .map(|magnitude| IntegerValue::Signed(-magnitude))
            }
        };
        match left_value.cmp(right_value) {
            Ordering::Less => {
                let (Some(lower), Some(upper)) = (numeral(left_value), numeral(right_value)) else {
                    return Ok(None);
                };
                let strict = self.literal_order(lower, upper)?;
                let left = self.math_term(left)?;
                let right = self.math_term(right)?;
                self.integer_law_application(
                    IntegerLaw::LessThanToLessOrEqual,
                    &[left, right, strict],
                )
                .map(Some)
            }
            Ordering::Equal => {
                let left = self.math_term(left)?;
                let right = self.math_term(right)?;
                let integer = self.integer_constant()?;
                let reflexive = self.arena.insert(Term::Refl {
                    ty: integer,
                    value: left,
                });
                self.integer_law_application(
                    IntegerLaw::EqualityToLessOrEqual,
                    &[left, right, reflexive],
                )
                .map(Some)
            }
            Ordering::Greater => {
                let Proposition::LessOrEqual(premise_left, premise_right) = premise else {
                    return Ok(None);
                };
                let (Some((_, left_value)), Some((_, right_value))) =
                    (premise_left.integer_value(), premise_right.integer_value())
                else {
                    return Ok(None);
                };
                let as_integer = |value: IntegerValue| match value {
                    IntegerValue::Signed(value) => BigInt::from_i128(value),
                    IntegerValue::Unsigned(value) => BigInt::from_u128(value),
                };
                if as_integer(left_value) <= as_integer(right_value) {
                    return Ok(None);
                }
                let strict = self.literal_order(right_value, left_value)?;
                let premise_left = self.fixed_scalar_term(premise_left)?;
                let premise_right = self.fixed_scalar_term(premise_right)?;
                let reflexive = self.integer_law_application(
                    IntegerLaw::LessThanLessOrEqualTransitivity,
                    &[premise_right, premise_left, premise_right, strict, evidence],
                )?;
                let scrutinee = self.irreflexive(premise_right, reflexive)?;
                let ty = self.denote(conclusion)?;
                Ok(Some(self.arena.insert(Term::EmptyElim { ty, scrutinee })))
            }
        }
    }
}
