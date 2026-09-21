//! Exact scalar subtraction in the shared mathematical integer vocabulary.
//!
//! Fixed assumptions describe subtraction by zero, self-subtraction, strict
//! and nonstrict antitonicity in its right operand, the two `add`/`sub`
//! order adjunctions `add a c ≤ b → a ≤ sub b c` and `b ≤ add a c → sub b c
//! ≤ a`, and the two-sided bound `a ≤ b → d ≤ c → sub a c ≤ sub b d`.
//! Their applications, followed by endpoint transport, prove
//! conditional decrease, unsigned nonnegativity, and the checked correlated
//! bounds `min ≤ l − r` and `l − r ≤ max` for open expressions. Once the
//! correlated sum has collapsed to its numeral `n`, the applicative
//! `add e r` the adjunction expects cannot match `n`, so the chain
//! substitutes the checked numeral-operation equation `add e r = n` — an
//! exact interned assumption — in reverse.
//!
//! The direct `IntegerAffineBound` subtract form cites no definition: its
//! premise is the conjunction of the two operand bounds and its
//! conclusion names the `sub` application itself. The left operand's
//! endpoint follows the conclusion's direction; the right operand's —
//! antitone — flips, so `x − y`'s lower bound spends `y`'s upper
//! endpoint. The same per-operand machinery as the add bounds re-shapes
//! each endpoint — an oriented `≤` stands, an `Equal` transports through
//! `eq_le`, a literal operand uses `refl`, and `Truth` cites its interned
//! carrier-membership bound — the two-sided law combines them, and the
//! checked `sub lb rb = k` numeral equation lands the literal. An
//! endpoint difference outside the representable numeral range keeps the
//! explicit instance fallback.
//! Overflowing scalar subtraction remains compositional.
//! The bounded checker still owns admissible carriers and exact premises.
//!
//! Representable closed scalar expressions retain their evaluated numeral
//! identity. Their order is derived from the binary numeral laws. A supplied
//! false positivity premise remains a valid conditional hypothesis: strict
//! irreflexivity and the core's empty elimination derive the conclusion.
//! Open mathematical subtraction shares this function when its children can
//! be denoted within the existing evaluation budget. An already-admitted open
//! expression with a resource-refused child retains its prior opaque identity;
//! this claims neither an evaluated value nor an arithmetic equality. Whole
//! closed expressions retain the evaluator's refusal. Nothing here evaluates
//! arbitrary open arithmetic or changes premise matching.

use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, IntegerSign, IntegerValue, Proposition, ScalarTerm,
};

use super::super::scheme_dsl::{self, apps, id, pi, scheme_at, v};
use super::{BoundedDenotationError, Declaration, Denotation, IntegerLaw, Term, TermHandle};
use crate::{ClosedIntegerEvaluator, kernel::KernelError, proof::ProofError};

#[cfg(test)]
mod bound_tests;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    Zero,
    Antitone,
    SelfZero,
    LessOrEqualAntitone,
    /// `add a c ≤ b → a ≤ sub b c` — the `≤`-adjunction cancelling an
    /// `add` on the relation's left endpoint.
    CancelAddLeft,
    /// `b ≤ add a c → sub b c ≤ a` — the same adjunction with the `add`
    /// on the right endpoint.
    CancelAddRight,
    /// `a ≤ b → d ≤ c → sub a c ≤ sub b d` — two-sided subtraction
    /// bound, monotone in its left operand and antitone in its right.
    MonotoneAntitone,
}

#[derive(Default)]
pub(super) struct Subtraction {
    operation: Option<u32>,
    laws: BTreeMap<Law, u32>,
    irreflexivity: Option<u32>,
    /// `sub l r = d` equations — numeral-operation bridges between a
    /// `subtract` application over evaluated operands and their denoted
    /// difference. Each is interned once by its checked `(l, r, d)`
    /// value triple.
    pub(super) numeral_differences: BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
}

impl Denotation {
    pub(super) fn subtract_operation(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.subtraction.operation {
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
        self.subtraction.operation = Some(position);
        Ok(position)
    }

    pub(super) fn subtract_terms(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.subtract_operation()?;
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

    fn subtract_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(&position) = self.subtraction.laws.get(&law) {
            position
        } else {
            let integer = self.integer()?;
            let operation = self.subtract_operation()?;
            let less_than = self.integer_less_than()?;
            let zero = self.binary_integer(false, 0)?;
            let constant = |position| scheme_at(position, Vec::new());
            let carrier = || constant(integer);
            let subtract = |left, right| apps(constant(operation), [left, right]);
            let lt = |left, right| apps(constant(less_than), [left, right]);
            let statement = match law {
                Law::SelfZero => pi(
                    "x",
                    carrier(),
                    id(carrier(), subtract(v("x"), v("x")), constant(zero)),
                ),
                Law::LessOrEqualAntitone => {
                    let less_or_equal = self.integer_less_or_equal()?;
                    let le = |left, right| apps(constant(less_or_equal), [left, right]);
                    pi(
                        "x",
                        carrier(),
                        pi(
                            "a",
                            carrier(),
                            pi(
                                "b",
                                carrier(),
                                pi(
                                    "_",
                                    le(v("a"), v("b")),
                                    le(subtract(v("x"), v("b")), subtract(v("x"), v("a"))),
                                ),
                            ),
                        ),
                    )
                }
                Law::MonotoneAntitone => {
                    let less_or_equal = self.integer_less_or_equal()?;
                    let le = |left, right| apps(constant(less_or_equal), [left, right]);
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
                                            le(v("d"), v("c")),
                                            le(subtract(v("a"), v("c")), subtract(v("b"), v("d"))),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    )
                }
                Law::CancelAddLeft | Law::CancelAddRight => {
                    let less_or_equal = self.integer_less_or_equal()?;
                    let add = self.add_operation()?;
                    let le = |left, right| apps(constant(less_or_equal), [left, right]);
                    let plus = |left, right| apps(constant(add), [left, right]);
                    let (premise, conclusion) = if law == Law::CancelAddLeft {
                        (
                            le(plus(v("a"), v("c")), v("b")),
                            le(v("a"), subtract(v("b"), v("c"))),
                        )
                    } else {
                        (
                            le(v("b"), plus(v("a"), v("c"))),
                            le(subtract(v("b"), v("c")), v("a")),
                        )
                    };
                    pi(
                        "a",
                        carrier(),
                        pi(
                            "b",
                            carrier(),
                            pi("c", carrier(), pi("_", premise, conclusion)),
                        ),
                    )
                }
                Law::Zero => pi(
                    "x",
                    carrier(),
                    id(carrier(), subtract(v("x"), constant(zero)), v("x")),
                ),
                Law::Antitone => pi(
                    "x",
                    carrier(),
                    pi(
                        "a",
                        carrier(),
                        pi(
                            "b",
                            carrier(),
                            pi(
                                "_",
                                lt(v("a"), v("b")),
                                lt(subtract(v("x"), v("b")), subtract(v("x"), v("a"))),
                            ),
                        ),
                    ),
                ),
            };
            let ty = scheme_dsl::build(&mut self.arena, &mut Vec::new(), &statement);
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.subtraction.laws.insert(law, position);
            position
        };
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    /// `forall x : Int, IntLt x x -> Empty`. This is an explicit fixed
    /// order assumption, not a new trusted inference or a numeral decision.
    pub(super) fn irreflexive(
        &mut self,
        value: TermHandle,
        evidence: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(position) = self.subtraction.irreflexivity {
            position
        } else {
            let integer = self.integer_constant()?;
            let relation_position = self.integer_less_than()?;
            let relation = self.constant(relation_position);
            let variable = self.arena.insert(Term::Variable(0));
            let function = self.arena.insert(Term::Apply {
                function: relation,
                argument: variable,
            });
            let domain = self.arena.insert(Term::Apply {
                function,
                argument: variable,
            });
            let empty = self.arena.insert(Term::Empty);
            let codomain = self.arena.insert(Term::Pi {
                domain,
                codomain: empty,
            });
            let ty = self.arena.insert(Term::Pi {
                domain: integer,
                codomain,
            });
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.subtraction.irreflexivity = Some(position);
            position
        };
        let function = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function,
            argument: value,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: evidence,
        }))
    }

    pub(super) fn subtraction_evidence(
        &mut self,
        difference: &Proposition,
        difference_evidence: TermHandle,
        positive_evidence: TermHandle,
        goal: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Proposition::Equal(
            result,
            subtraction @ ScalarTerm::ExactIntegerSubtract { left, right, .. },
        ) = difference
        else {
            unreachable!("bounded subtraction checker ran first")
        };
        let result_term = self.fixed_scalar_term(result)?;
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let subtraction_term = self.fixed_scalar_term(subtraction)?;
        let zero = self.math_term(&IntegerMathTerm::literal(IntegerValue::Unsigned(0)))?;
        let decrease = if let Some((_, value)) = subtraction.integer_value() {
            let (_, decrement) = right
                .integer_value()
                .expect("evaluated exact subtraction has evaluated operands");
            if matches!(decrement, IntegerValue::Signed(value) if value <= 0)
                || decrement == IntegerValue::Unsigned(0)
            {
                let contradiction = if decrement == IntegerValue::Signed(0)
                    || decrement == IntegerValue::Unsigned(0)
                {
                    positive_evidence
                } else {
                    let backwards = self.literal_order(decrement, IntegerValue::Signed(0))?;
                    self.integer_law_application(
                        IntegerLaw::LessThanTransitivity,
                        &[zero, right_term, zero, positive_evidence, backwards],
                    )?
                };
                let scrutinee = self.irreflexive(zero, contradiction)?;
                let ty = self.denote(goal)?;
                return Ok(self.arena.insert(Term::EmptyElim { ty, scrutinee }));
            }
            let (_, original) = left
                .integer_value()
                .expect("evaluated exact subtraction has evaluated operands");
            self.literal_order(value, original)?
        } else {
            let subtract_zero = self.subtract_terms(left_term, zero)?;
            let decrease = self.subtract_law_application(
                Law::Antitone,
                &[left_term, zero, right_term, positive_evidence],
            )?;
            let equality = self.subtract_law_application(Law::Zero, &[left_term])?;
            self.integer_law_application(
                IntegerLaw::LessThanSubstituteRight,
                &[
                    subtraction_term,
                    subtract_zero,
                    left_term,
                    equality,
                    decrease,
                ],
            )?
        };
        let integer = self.integer_constant()?;
        let equality = self.symmetry(integer, result_term, subtraction_term, difference_evidence);
        self.integer_law_application(
            IntegerLaw::LessThanSubstituteLeft,
            &[subtraction_term, result_term, left_term, equality, decrease],
        )
    }

    /// Elaborate the already-checked CorrelatedUnsignedSubtract form only.
    /// The common witness checker remains the admission authority; this
    /// selection does not change its numeric, citation or endpoint matching.
    pub(super) fn unsigned_subtract_bound_evidence(
        &mut self,
        premise: &Proposition,
        evidence: TermHandle,
        witness: &crate::IntegerAffineWitness,
        goal: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } = &witness.target
        else {
            return Ok(None);
        };
        let direct = |term: &ScalarTerm| {
            matches!(term, ScalarTerm::Value { .. } | ScalarTerm::Integer { .. })
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed
            || scalar_type.sign() != IntegerSign::Unsigned
            || !witness.definition_axioms.is_empty()
            || !witness.literal_axioms.is_empty()
            || &witness.root != right.as_ref()
            || left == right
            || !direct(left)
            || !direct(right)
            || premise != &Proposition::LessOrEqual(right.as_ref().clone(), left.as_ref().clone())
        {
            return Ok(None);
        }
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let zero = self.math_term(&IntegerMathTerm::literal(IntegerValue::Unsigned(0)))?;
        let goal_type = self.denote(goal)?;
        let Some(super::IntegerRelation::LessOrEqual {
            right: difference, ..
        }) = self.integer_relation(goal_type)
        else {
            unreachable!("checked unsigned subtraction bound denotes IntLe")
        };
        if let (
            Some((_, IntegerValue::Unsigned(left_value))),
            Some((_, IntegerValue::Unsigned(right_value))),
        ) = (left.integer_value(), right.integer_value())
        {
            if left_value < right_value {
                // A false closed premise is still a valid conditional proof.
                let order = self.literal_order(
                    IntegerValue::Unsigned(left_value),
                    IntegerValue::Unsigned(right_value),
                )?;
                let contradiction = self.integer_law_application(
                    super::IntegerLaw::LessThanLessOrEqualTransitivity,
                    &[left_term, right_term, left_term, order, evidence],
                )?;
                let scrutinee = self.irreflexive(left_term, contradiction)?;
                return Ok(Some(self.arena.insert(Term::EmptyElim {
                    ty: goal_type,
                    scrutinee,
                })));
            }
            let result = left_value - right_value;
            // Distinct direct literals cannot subtract to zero in this checked family.
            let strict =
                self.literal_order(IntegerValue::Unsigned(0), IntegerValue::Unsigned(result))?;
            let order = self.integer_law_application(
                super::IntegerLaw::LessThanToLessOrEqual,
                &[zero, difference, strict],
            )?;
            return Ok(Some(order));
        }
        let self_difference = self.subtract_terms(left_term, left_term)?;
        let order = self.subtract_law_application(
            Law::LessOrEqualAntitone,
            &[left_term, right_term, left_term, evidence],
        )?;
        let equality = self.subtract_law_application(Law::SelfZero, &[left_term])?;
        self.integer_law_application(
            super::IntegerLaw::LessOrEqualSubstituteLeft,
            &[self_difference, zero, difference, equality, order],
        )
        .map(Some)
    }

    /// Select a checked endpoint-plus-addend witness — `root = e + r` —
    /// whose target is the difference `l - r`. The caller has already run
    /// affine_bound_relation, including citation checks. Integer equality
    /// normalization may reorder authored `Equal` endpoints; each
    /// transport therefore orients the denoted Id, not the source pair.
    /// Lower and upper bounds use the same add-cancel laws: only the
    /// inequality endpoint being transported changes. Select that
    /// endpoint from the checked conclusion and require its exact carrier
    /// minimum/maximum, so a convenient addition cannot replace the
    /// witness.
    pub(super) fn correlated_subtract_bound_evidence(
        &mut self,
        premise: &Proposition,
        mut evidence: TermHandle,
        witness: &crate::IntegerAffineWitness,
        conclusion: &Proposition,
        definitions: &[(Proposition, TermHandle)],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::ExactIntegerSubtract {
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
        let (lower, bound, difference) = match conclusion {
            Proposition::IntegerMathLessOrEqual(
                bound,
                difference @ IntegerMathTerm::Subtract(..),
            ) => (true, bound, difference),
            Proposition::IntegerMathLessOrEqual(
                difference @ IntegerMathTerm::Subtract(..),
                bound,
            ) => (false, bound, difference),
            _ => return Ok(None),
        };
        // The checked conclusion's bound side is the carrier endpoint
        // literal, so it always evaluates. When the difference side still
        // has a canonical numeral the goal is a decidable `IntLe` between
        // numerals, discharged by the numeral laws rather than an
        // instance axiom; an open difference keeps the adjunction chain
        // below.
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
        let difference_value = evaluate(difference)?;
        if let (Some(bound_value), Some(difference_value)) = (&bound_value, &difference_value) {
            let (goal_left, goal_right, left_value, right_value) = if lower {
                (bound, difference, bound_value, difference_value)
            } else {
                (difference, bound, difference_value, bound_value)
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
        if difference_value.is_some() {
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
        let (expression, equality) =
            if witness.definition_axioms.is_empty() && witness.literal_axioms.is_empty() {
                (&witness.root, None)
            } else if witness.definition_axioms.len() == 1 {
                let Some((expression, proof, proposition)) = equality_for(&witness.root) else {
                    return Ok(None);
                };
                (expression, Some((proof, proposition)))
            } else {
                return Ok(None);
            };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type: add_type,
            left: endpoint,
            right: addend,
        } = expression
        else {
            return Ok(None);
        };
        let expected_premise = if lower {
            Proposition::LessOrEqual(witness.root.clone(), left.as_ref().clone())
        } else {
            Proposition::LessOrEqual(left.as_ref().clone(), witness.root.clone())
        };
        if add_type != scalar_type || addend != right || premise != &expected_premise {
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
        let expression_term = self.fixed_scalar_term(expression)?;
        if let Some((equality, proposition)) = equality {
            let root_term = self.fixed_scalar_term(&witness.root)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) =
                self.directed_equality(from, to, root_term, expression_term, equality)
            else {
                return Ok(None);
            };
            evidence = if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[root_term, expression_term, left_term, equality, evidence],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left_term, root_term, expression_term, equality, evidence],
                )?
            };
        }
        // The premise now bounds the denoted sum expression. A closed sum
        // denotes to its numeral `n` rather than the applicative
        // `add e' r'` the adjunction expects, so that chain substitutes
        // the checked numeral-operation equation `add e r = n` — an exact
        // interned assumption — in reverse; an open sum is already the
        // applicative form.
        let sum = self.add_terms(endpoint_term, right_term)?;
        if !self.arena.structurally_equal(expression_term, sum) {
            let (Some((_, endpoint)), Some((_, addend)), Some((_, total))) = (
                endpoint.integer_value(),
                right.integer_value(),
                expression.integer_value(),
            ) else {
                return Ok(None);
            };
            let Some(equation) = self.numeral_sum(
                endpoint,
                addend,
                total,
                endpoint_term,
                right_term,
                expression_term,
            )?
            else {
                return Ok(None);
            };
            let integer = self.integer_constant()?;
            let equation = self.symmetry(integer, sum, expression_term, equation);
            evidence = if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[expression_term, sum, left_term, equation, evidence],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left_term, expression_term, sum, equation, evidence],
                )?
            };
        }
        let order = if lower {
            self.subtract_law_application(
                Law::CancelAddLeft,
                &[endpoint_term, left_term, right_term, evidence],
            )?
        } else {
            self.subtract_law_application(
                Law::CancelAddRight,
                &[endpoint_term, left_term, right_term, evidence],
            )?
        };
        if let Some((equality, proposition)) = endpoint_equality {
            let literal = self.fixed_scalar_term(&endpoint_literal)?;
            let difference = self.subtract_terms(left_term, right_term)?;
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
                    &[endpoint_term, literal, difference, equality, order],
                )
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[difference, endpoint_term, literal, equality, order],
                )
            }
            .map(Some)
        } else {
            Ok(Some(order))
        }
    }

    /// `Id Int (sub l r) d` — the numeral-operation equation bridging a
    /// `subtract` application over evaluated operands to their denoted
    /// difference. Interned once per checked `(l, r, d)` value triple:
    /// the signature then names that exact equation in place of a
    /// whole-rule implication. `None` when the values do not satisfy
    /// `l - r = d` — a defensive refusal, since the caller derives them
    /// from a checked direct subtraction — so a miss keeps the instance
    /// fallback rather than naming a false equation.
    pub(super) fn numeral_difference(
        &mut self,
        minuend: IntegerValue,
        subtrahend: IntegerValue,
        difference: IntegerValue,
        minuend_term: TermHandle,
        subtrahend_term: TermHandle,
        difference_term: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let as_integer = |value: IntegerValue| match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        if as_integer(minuend).sub(&as_integer(subtrahend)) != as_integer(difference) {
            return Ok(None);
        }
        if let Some(&position) = self
            .subtraction
            .numeral_differences
            .get(&(minuend, subtrahend, difference))
        {
            return Ok(Some(self.constant(position)));
        }
        let integer = self.integer_constant()?;
        let application = self.subtract_terms(minuend_term, subtrahend_term)?;
        let ty = self.arena.insert(Term::Id {
            ty: integer,
            left: application,
            right: difference_term,
        });
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.subtraction
            .numeral_differences
            .insert((minuend, subtrahend, difference), position);
        Ok(Some(self.constant(position)))
    }

    /// Denote the checked direct-subtract bound: a `Conjunction` of the
    /// two operand bounds proves `k ≤ sub l r` or `sub l r ≤ k`, where
    /// `k` is the checked endpoint difference. Subtraction is antitone in
    /// its right operand, so the right bound re-shapes to the direction
    /// opposite the conclusion's — `lb − ub ≤ l − r` needs `r`'s upper
    /// endpoint. The same endpoint machinery as the add bounds applies —
    /// an oriented `≤` stands, an `Equal` transports through `eq_le`, a
    /// literal operand uses `refl`, `Truth` cites its interned
    /// carrier-membership bound — the two-sided
    /// `a ≤ b → d ≤ c → sub a c ≤ sub b d` law combines them, and the
    /// checked `sub lb rb = k` numeral equation lands the literal. A
    /// bound literal outside the representable numeral range keeps the
    /// explicit instance fallback.
    pub(super) fn direct_subtract_bound_evidence(
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
        let ScalarTerm::ExactIntegerSubtract {
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
        let (lower, bound, difference) = match conclusion {
            Proposition::IntegerMathLessOrEqual(
                bound,
                difference @ IntegerMathTerm::Subtract(..),
            ) => (true, bound, difference),
            Proposition::IntegerMathLessOrEqual(
                difference @ IntegerMathTerm::Subtract(..),
                bound,
            ) => (false, bound, difference),
            _ => return Ok(None),
        };
        // The mapped bound is a canonical math literal; when its
        // magnitude leaves the representable numeral range the denoted
        // endpoint is an opaque `Int` constant no checked
        // `sub lb rb = k` equation can name — keep the instance fallback.
        let IntegerMathTerm::IntegerLiteral(bound_literal) = bound else {
            return Ok(None);
        };
        let Some(bound_value) = super::math_literal_value(*bound_literal) else {
            return Ok(None);
        };
        // The conjunction denotes to a `Σ` pair: the first conjunct is
        // `fst`, the second — last — conjunct is `snd`.
        let left_evidence = self.arena.insert(Term::Fst { pair: evidence });
        let right_evidence = self.arena.insert(Term::Snd { pair: evidence });
        // The left operand is monotone — its endpoint direction is the
        // conclusion's; the right operand is antitone — `l − r`'s lower
        // bound spends `r`'s upper endpoint, so the direction flips.
        let Some((left_endpoint, left_order)) =
            self.add_bound_endpoint(left, left_bound, left_evidence, lower, scalar_type)?
        else {
            return Ok(None);
        };
        let Some((right_endpoint, right_order)) =
            self.add_bound_endpoint(right, right_bound, right_evidence, !lower, scalar_type)?
        else {
            return Ok(None);
        };
        let left_endpoint_term = self.fixed_scalar_term(&left_endpoint)?;
        let right_endpoint_term = self.fixed_scalar_term(&right_endpoint)?;
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let bound_difference = self.subtract_terms(left_endpoint_term, right_endpoint_term)?;
        let operand_difference = self.subtract_terms(left_term, right_term)?;
        // The conclusion's `Subtract` math term must denote the same
        // application as the target's operands — the shared relation
        // already equated them, so this is the defensive shape check.
        let difference_term = self.math_term(difference)?;
        if !self
            .arena
            .structurally_equal(difference_term, operand_difference)
        {
            return Ok(None);
        }
        // `sub lb_l rb_r ≤ sub l r` (lower) or
        // `sub l r ≤ sub ub_l lb_r` (upper).
        let order = if lower {
            self.subtract_law_application(
                Law::MonotoneAntitone,
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
            self.subtract_law_application(
                Law::MonotoneAntitone,
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
        let Some(equality) = self.numeral_difference(
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
                &[
                    bound_difference,
                    bound_term,
                    operand_difference,
                    equality,
                    order,
                ],
            )
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualSubstituteRight,
                &[
                    operand_difference,
                    bound_difference,
                    bound_term,
                    equality,
                    order,
                ],
            )
        }
        .map(Some)
    }
}
