//! Shared integer addition and checked correlated bound derivations.
//!
//! Addition monotonicity and subtraction cancellation are fixed assumptions,
//! not per-instance conclusions. The original affine witness checker runs
//! first. A conclusion whose endpoints both evaluate inside `fixed_magnitude`
//! is `IntLe` between two canonical numerals: the binary numeral laws decide
//! a true relation outright, and a false one is emptied through a checked
//! false premise. Sums still open keep the monotonicity and cancellation
//! chain; cases that need a numeral-to-operation equation — a closed
//! difference under an open sum, or a sum outside the fixed literal range —
//! retain the existing explicit instance fallback rather than claiming that
//! arithmetic laws are definitional.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::{IntegerCarrier, IntegerMathTerm, IntegerValue, Proposition, ScalarTerm};

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
}

#[derive(Default)]
pub(super) struct Addition {
    operation: Option<u32>,
    laws: BTreeMap<Law, u32>,
}

impl Denotation {
    fn add_operation(&mut self) -> Result<u32, BoundedDenotationError> {
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
        // axiom; an open sum keeps the law chain below only while the
        // difference stays an unreduced `subtract` application.
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
            return self.closed_add_bound_evidence(
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
        // The cancellation transport matches `add (subtract endpoint right)
        // right` against the difference denotation, which collapses to a
        // numeral once the difference is closed; that goal needs a
        // numeral-operation equation, so it keeps the instance fallback.
        if difference.integer_value().is_some() {
            return Ok(None);
        }
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
        let equality =
            self.add_law_application(Law::CancelSubtract, &[endpoint_term, right_term])?;
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

    /// The goal is `IntLe` between two canonical constants. A strict
    /// relation between fixed-width numerals is decided by the binary
    /// numeral laws; equal endpoints are `refl`; a false one is discharged
    /// by empty elimination through the checked premise when it is itself
    /// a false closed inequality. Strict endpoints outside `IntegerValue`'s
    /// range — or a premise without a closed contradiction — keep the
    /// caller's instance fallback.
    fn closed_add_bound_evidence(
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
