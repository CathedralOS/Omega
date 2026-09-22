//! Exact mathematical multiplication in the shared `Int` vocabulary.
//!
//! `mul` denotes an open `IntegerMathTerm::Multiply` applicatively — the
//! same way `add`/`sub` denote their nodes — so the checked correlated
//! bounds `min ≤ l·r` and `l·r ≤ max` compose from fixed order laws
//! rather than a per-instance rule axiom. Six fixed `Π` assumptions
//! suffice: multiplication by a positive right operand is monotone and by
//! a negative one antitone, and the per-type truncating-division residual
//! `n ≤ mul (div_T n d) d` (for `n ≤ 0`) or `mul (div_T n d) d ≤ n`
//! (for `0 ≤ n`) re-expresses the carrier endpoint through the quotient
//! the source recorded. `div_T` is the same uninterpreted
//! `ExactIntegerDivide(T)` denotation the scalar term already carries;
//! the residual laws are exact statements of truncating division's
//! endpoint behavior, audited once like the other fixed laws.
//!
//! A checked `CorrelatedMultiply{Minimum,Maximum}` witness supplies
//! `root = endpoint / r` — either the divide term itself or a cited
//! definition axiom — and evidence `⟨sign, bound⟩` with `1 ≤ r` or
//! `r ≤ −2`. The sign selects the monotone/antitone law applied to the
//! bound's denoted endpoint relation `div_app R l'`; the residual law at
//! the denoted endpoint supplies `endpoint R mul div_app r'`; one
//! transitivity composes them into the conclusion. A landed endpoint
//! value substitutes through its cited literal equality on the law's `n`
//! argument and on the final `IntLe` endpoint, exactly like the
//! correlated subtract chain. Shapes outside this vocabulary keep the
//! existing per-instance `rule_axiom` fallback unchanged — the checked
//! relation already owns admissibility.

use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
    ScalarTerm,
};

use super::super::scheme_dsl::{self, apps, pi, scheme_at, v};
use super::integer_operations::IntegerOperation;
use super::{BoundedDenotationError, Declaration, Denotation, IntegerLaw, Term, TermHandle};

#[cfg(test)]
mod tests;

/// `mul`'s fixed order laws and the per-type truncating-division
/// residuals — one assumption constant each, with an exact `Π` statement
/// over the shared `Int` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    /// `a ≤ b → 1 ≤ r → mul a r ≤ mul b r` — right multiplication by a
    /// positive operand preserves order.
    MonotonePositive,
    /// `a ≤ b → r ≤ −1 → mul b r ≤ mul a r` — right multiplication by a
    /// negative operand reverses order.
    AntitoneNegative,
    /// `n ≤ 0 → 1 ≤ d → n ≤ mul (div_T n d) d` — truncating division of
    /// a non-positive dividend by a positive divisor keeps the product
    /// at or above the dividend.
    DivideLowerPositive(IntegerType),
    /// `n ≤ 0 → d ≤ −1 → n ≤ mul (div_T n d) d` — truncating division of
    /// a non-positive dividend by a negative divisor keeps the product
    /// at or above the dividend.
    DivideLowerNegative(IntegerType),
    /// `0 ≤ n → 1 ≤ d → mul (div_T n d) d ≤ n` — truncating division of
    /// a non-negative dividend by a positive divisor keeps the product
    /// at or below the dividend.
    DivideUpperPositive(IntegerType),
    /// `0 ≤ n → d ≤ −1 → mul (div_T n d) d ≤ n` — truncating division of
    /// a non-negative dividend by a negative divisor keeps the product
    /// at or below the dividend.
    DivideUpperNegative(IntegerType),
}

#[derive(Default)]
pub(super) struct Multiplication {
    operation: Option<u32>,
    laws: BTreeMap<Law, u32>,
}

impl Denotation {
    /// `mul : Π(_ : Int). Π(_ : Int). Int` — the shared multiplication
    /// every open `IntegerMathTerm::Multiply` denotes, so the fixed
    /// order laws quantify over its applications.
    pub(super) fn multiply_operation(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.multiplication.operation {
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
        self.multiplication.operation = Some(position);
        Ok(position)
    }

    /// `mul l r` — the applicative denotation an open mathematical
    /// product keeps so the order laws and transport can name it.
    pub(super) fn multiply_terms(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.multiply_operation()?;
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

    /// `law a₁ … aₙ` — one multiplication-roster constant applied to the
    /// denoted endpoints and premise evidence, in statement order.
    fn multiply_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = if let Some(&position) = self.multiplication.laws.get(&law) {
            position
        } else {
            let integer = self.integer()?;
            let operation = self.multiply_operation()?;
            let less_or_equal = self.integer_less_or_equal()?;
            let zero = self.binary_integer(false, 0)?;
            let one = self.binary_integer(false, 1)?;
            let negative_one = self.binary_integer(true, 1)?;
            let constant = |position| scheme_at(position, Vec::new());
            let carrier = || constant(integer);
            let le = |left, right| apps(constant(less_or_equal), [left, right]);
            let mul = |left, right| apps(constant(operation), [left, right]);
            let statement = match law {
                Law::MonotonePositive => pi(
                    "a",
                    carrier(),
                    pi(
                        "b",
                        carrier(),
                        pi(
                            "r",
                            carrier(),
                            pi(
                                "_",
                                le(v("a"), v("b")),
                                pi(
                                    "_",
                                    le(constant(one), v("r")),
                                    le(mul(v("a"), v("r")), mul(v("b"), v("r"))),
                                ),
                            ),
                        ),
                    ),
                ),
                Law::AntitoneNegative => pi(
                    "a",
                    carrier(),
                    pi(
                        "b",
                        carrier(),
                        pi(
                            "r",
                            carrier(),
                            pi(
                                "_",
                                le(v("a"), v("b")),
                                pi(
                                    "_",
                                    le(v("r"), constant(negative_one)),
                                    le(mul(v("b"), v("r")), mul(v("a"), v("r"))),
                                ),
                            ),
                        ),
                    ),
                ),
                Law::DivideLowerPositive(scalar_type)
                | Law::DivideLowerNegative(scalar_type)
                | Law::DivideUpperPositive(scalar_type)
                | Law::DivideUpperNegative(scalar_type) => {
                    let divide =
                        self.integer_operation(IntegerOperation::ExactDivide(scalar_type))?;
                    let quotient = |left, right| apps(constant(divide), [left, right]);
                    let lower_endpoint = matches!(
                        law,
                        Law::DivideLowerPositive(_) | Law::DivideLowerNegative(_)
                    );
                    let positive_divisor = matches!(
                        law,
                        Law::DivideLowerPositive(_) | Law::DivideUpperPositive(_)
                    );
                    let dividend = if lower_endpoint {
                        le(v("n"), constant(zero))
                    } else {
                        le(constant(zero), v("n"))
                    };
                    let divisor = if positive_divisor {
                        le(constant(one), v("d"))
                    } else {
                        le(v("d"), constant(negative_one))
                    };
                    let product = mul(quotient(v("n"), v("d")), v("d"));
                    let residual = if lower_endpoint {
                        le(v("n"), product)
                    } else {
                        le(product, v("n"))
                    };
                    pi(
                        "n",
                        carrier(),
                        pi(
                            "d",
                            carrier(),
                            pi("_", dividend, pi("_", divisor, residual)),
                        ),
                    )
                }
            };
            let ty = scheme_dsl::build(&mut self.arena, &mut Vec::new(), &statement);
            let position = self.position()?;
            self.declarations.push(Declaration::assumption(0, ty));
            self.multiplication.laws.insert(law, position);
            position
        };
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    /// `IntLe l' u'` evidence between two closed numerals: `refl` +
    /// `eq_le` when they agree, the binary-order strict laws plus
    /// `lt_le` when `l < u`. `None` when `l > u` — a defensive refusal,
    /// since callers derive the pair from an already-checked relation.
    fn integer_le_numeral(
        &mut self,
        lower: IntegerValue,
        upper: IntegerValue,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let parts = |value| match value {
            IntegerValue::Signed(value) => (value < 0, value.unsigned_abs()),
            IntegerValue::Unsigned(value) => (false, value),
        };
        let (negative_lower, lower_magnitude) = parts(lower);
        let (negative_upper, upper_magnitude) = parts(upper);
        let lower_term = {
            let position = self.binary_integer(negative_lower, lower_magnitude)?;
            self.constant(position)
        };
        let upper_term = {
            let position = self.binary_integer(negative_upper, upper_magnitude)?;
            self.constant(position)
        };
        if lower == upper {
            let integer = self.integer_constant()?;
            let reflexive = self.arena.insert(Term::Refl {
                ty: integer,
                value: lower_term,
            });
            return self
                .integer_law_application(
                    IntegerLaw::EqualityToLessOrEqual,
                    &[lower_term, upper_term, reflexive],
                )
                .map(Some);
        }
        let as_integer = |value: IntegerValue| match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        if as_integer(lower) >= as_integer(upper) {
            return Ok(None);
        }
        let strict = self.literal_order(lower, upper)?;
        self.integer_law_application(
            IntegerLaw::LessThanToLessOrEqual,
            &[lower_term, upper_term, strict],
        )
        .map(Some)
    }

    /// Elaborate the checked `CorrelatedMultiply{Minimum,Maximum}` bound:
    /// premise `Conjunction[sign, bound]` under a witness whose
    /// `root = endpoint / r` relates the product `l·r` to the carrier
    /// endpoint through the exact quotient. `1 ≤ r` selects the monotone
    /// law and `r ≤ −2` the antitone one; the per-type
    /// truncating-division residual supplies the endpoint's side of the
    /// transitivity. `None` keeps the per-instance `rule_axiom` fallback
    /// for every shape this fixed vocabulary does not name.
    pub(super) fn correlated_multiply_bound_evidence(
        &mut self,
        premise: &Proposition,
        evidence: TermHandle,
        witness: &crate::IntegerAffineWitness,
        conclusion: &Proposition,
        definitions: &[(Proposition, TermHandle)],
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let ScalarTerm::ExactIntegerMultiply {
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
        let (lower, bound, product) = match conclusion {
            Proposition::IntegerMathLessOrEqual(bound, product @ IntegerMathTerm::Multiply(..)) => {
                (true, bound, product)
            }
            Proposition::IntegerMathLessOrEqual(product @ IntegerMathTerm::Multiply(..), bound) => {
                (false, bound, product)
            }
            _ => return Ok(None),
        };
        let IntegerMathTerm::IntegerLiteral(bound_literal) = bound else {
            return Ok(None);
        };
        let endpoint_value = if lower {
            scalar_type.minimum_value()
        } else {
            scalar_type.maximum_value()
        };
        // Math literals canonicalize non-negative values to `Unsigned`,
        // so the endpoint comparison is numeric, not by variant.
        let as_integer = |value: IntegerValue| match value {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        if super::math_literal_value(*bound_literal).map(as_integer)
            != Some(as_integer(endpoint_value))
        {
            return Ok(None);
        }
        let Proposition::Conjunction(parts) = premise else {
            return Ok(None);
        };
        let [sign_evidence, bound_evidence] = parts.as_slice() else {
            return Ok(None);
        };
        let one = ScalarTerm::integer(
            *scalar_type,
            match scalar_type.sign() {
                IntegerSign::Signed => IntegerValue::Signed(1),
                IntegerSign::Unsigned => IntegerValue::Unsigned(1),
            },
        )
        .expect("one is admitted by every fixed carrier");
        let negative_two = ScalarTerm::integer(*scalar_type, IntegerValue::Signed(-2)).ok();
        let positive = sign_evidence == &Proposition::LessOrEqual(one, right.as_ref().clone());
        let negative = negative_two.is_some_and(|negative_two| {
            sign_evidence == &Proposition::LessOrEqual(right.as_ref().clone(), negative_two)
        });
        if !positive && !negative {
            return Ok(None);
        }
        let expected_bound = if lower == positive {
            Proposition::LessOrEqual(witness.root.clone(), left.as_ref().clone())
        } else {
            Proposition::LessOrEqual(left.as_ref().clone(), witness.root.clone())
        };
        if bound_evidence != &expected_bound {
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
        let (expression, definition) =
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
        let ScalarTerm::ExactIntegerDivide {
            scalar_type: divide_type,
            left: endpoint,
            right: divisor,
        } = expression
        else {
            return Ok(None);
        };
        if divide_type != scalar_type || divisor.as_ref() != right.as_ref() {
            return Ok(None);
        }
        let endpoint_literal = ScalarTerm::integer(*scalar_type, endpoint_value)
            .expect("carrier endpoint is representable");
        let endpoint_equality = if endpoint.integer_value() == Some((*scalar_type, endpoint_value))
        {
            None
        } else {
            let Some((literal, proof, proposition)) = equality_for(endpoint) else {
                return Ok(None);
            };
            if literal.integer_value() != Some((*scalar_type, endpoint_value)) {
                return Ok(None);
            }
            Some((literal, proof, proposition))
        };
        let left_term = self.fixed_scalar_term(left)?;
        let right_term = self.fixed_scalar_term(right)?;
        let endpoint_term = self.fixed_scalar_term(endpoint)?;
        let expression_term = self.fixed_scalar_term(expression)?;
        let bound_term = self.math_term(bound)?;
        let endpoint_bound_term = self.fixed_scalar_term(&endpoint_literal)?;
        if !self
            .arena
            .structurally_equal(bound_term, endpoint_bound_term)
        {
            return Ok(None);
        }
        let product_term = self.math_term(product)?;
        let operand_product = self.multiply_terms(left_term, right_term)?;
        if !self.arena.structurally_equal(product_term, operand_product) {
            return Ok(None);
        }
        // The premise denotes `Σ(⟦sign⟧, ⟦bound⟧)`: `sign` is the
        // divisor-side order and `bound` the endpoint's relation to `l'`.
        let sign = self.arena.insert(Term::Fst { pair: evidence });
        let mut bound = self.arena.insert(Term::Snd { pair: evidence });
        // Transport `root'` to the denoted quotient through the cited
        // definition when the root is a separate value.
        if let Some((equality, proposition)) = definition {
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
            bound = if lower == positive {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[root_term, expression_term, left_term, equality, bound],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[left_term, root_term, expression_term, equality, bound],
                )?
            };
        }
        let zero = {
            let position = self.binary_integer(false, 0)?;
            self.constant(position)
        };
        // `sign_term` is `IntLe 1' r'` or `IntLe r' (−1')`: the checked
        // negative sign names `r ≤ −2`, transported through `−2 ≤ −1`.
        let sign_term = if positive {
            sign
        } else {
            let negative_one_term = {
                let position = self.binary_integer(true, 1)?;
                self.constant(position)
            };
            let negative_two_term = {
                let position = self.binary_integer(true, 2)?;
                self.constant(position)
            };
            let strict = self.literal_order(IntegerValue::Signed(-2), IntegerValue::Signed(-1))?;
            let step = self.integer_law_application(
                IntegerLaw::LessThanToLessOrEqual,
                &[negative_two_term, negative_one_term, strict],
            )?;
            self.integer_law_application(
                IntegerLaw::LessOrEqualTransitivity,
                &[right_term, negative_two_term, negative_one_term, sign, step],
            )?
        };
        // `endpoint_side` is `IntLe e' 0'` for a minimum endpoint or
        // `IntLe 0' e'` for a maximum — derived on the endpoint numeral
        // and transported to a landed endpoint value through its cited
        // literal equality.
        let Some(numeral_side) = (if lower {
            self.integer_le_numeral(endpoint_value, IntegerValue::Unsigned(0))
        } else {
            self.integer_le_numeral(IntegerValue::Unsigned(0), endpoint_value)
        })?
        else {
            return Ok(None);
        };
        let endpoint_side = if let Some((literal, equality, proposition)) = endpoint_equality {
            let literal_term = self.fixed_scalar_term(literal)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) =
                self.directed_equality(from, to, literal_term, endpoint_term, equality)
            else {
                return Ok(None);
            };
            if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[literal_term, endpoint_term, zero, equality, numeral_side],
                )?
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[zero, literal_term, endpoint_term, equality, numeral_side],
                )?
            }
        } else {
            numeral_side
        };
        // The residual: `IntLe e' (mul div_app r')` for a minimum or
        // `IntLe (mul div_app r') e'` for a maximum.
        let residual = self.multiply_law_application(
            match (lower, positive) {
                (true, true) => Law::DivideLowerPositive(*scalar_type),
                (true, false) => Law::DivideLowerNegative(*scalar_type),
                (false, true) => Law::DivideUpperPositive(*scalar_type),
                (false, false) => Law::DivideUpperNegative(*scalar_type),
            },
            &[endpoint_term, right_term, endpoint_side, sign_term],
        )?;
        // `bound` reads `IntLe a b` in the law's argument order — the
        // monotone law lifts it to `mul a r' ≤ mul b r'`, the antitone
        // one to `mul b r' ≤ mul a r'`.
        let (a, b) = if lower == positive {
            (expression_term, left_term)
        } else {
            (left_term, expression_term)
        };
        let monotone = self.multiply_law_application(
            if positive {
                Law::MonotonePositive
            } else {
                Law::AntitoneNegative
            },
            &[a, b, right_term, bound, sign_term],
        )?;
        let endpoint_product = self.multiply_terms(expression_term, right_term)?;
        let combined = if lower {
            self.integer_law_application(
                IntegerLaw::LessOrEqualTransitivity,
                &[
                    endpoint_term,
                    endpoint_product,
                    operand_product,
                    residual,
                    monotone,
                ],
            )?
        } else {
            self.integer_law_application(
                IntegerLaw::LessOrEqualTransitivity,
                &[
                    operand_product,
                    endpoint_product,
                    endpoint_term,
                    monotone,
                    residual,
                ],
            )?
        };
        // A landed endpoint value substitutes to its literal on the
        // conclusion's bound endpoint; a literal endpoint already agrees.
        if let Some((literal, equality, proposition)) = endpoint_equality {
            let literal_term = self.fixed_scalar_term(literal)?;
            let denoted = self.denote(proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(equality) =
                self.directed_equality(from, to, endpoint_term, literal_term, equality)
            else {
                return Ok(None);
            };
            if lower {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteLeft,
                    &[
                        endpoint_term,
                        literal_term,
                        operand_product,
                        equality,
                        combined,
                    ],
                )
            } else {
                self.integer_law_application(
                    IntegerLaw::LessOrEqualSubstituteRight,
                    &[
                        operand_product,
                        endpoint_term,
                        literal_term,
                        equality,
                        combined,
                    ],
                )
            }
            .map(Some)
        } else {
            Ok(Some(combined))
        }
    }
}
