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

use semantic_vocabulary::{
    IntegerCarrier, IntegerMathTerm, IntegerType, IntegerValue, Proposition, ScalarTerm,
};

use super::super::scheme_dsl::{self, apps, id, pi, pis, scheme_at, v};
use super::denotation::math_less_or_equal_sides;
use super::integer_operations::IntegerOperation;
use super::{
    BoundedDenotationError, Denotation, IntegerLaw, Term, TermHandle,
    affine_witness_root_expression, unbounded_integer,
};

#[cfg(test)]
mod tests;

/// `mul`'s fixed order laws and the per-type truncating-division
/// residuals — one assumption constant each, with an exact `Π` statement
/// over the shared `Int` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    /// Standard mathematical integer distributivity. This fixed universal
    /// statement is retained explicitly, never a source goal instance.
    DistributeAdd,
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
    /// `mul l r = p` equations — numeral-operation bridges between a
    /// `mul` application over evaluated operands and their denoted
    /// product, interned once per checked `(l, r, p)` value triple.
    pub(super) numeral_products: BTreeMap<(IntegerValue, IntegerValue, IntegerValue), u32>,
    /// `div_T n d = q` equations — the same numeral-operation bridge for
    /// the per-type truncating-division application, interned once per
    /// checked `(T, n, d, q)` quadruple.
    pub(super) numeral_quotients:
        BTreeMap<(IntegerType, IntegerValue, IntegerValue, IntegerValue), u32>,
}

impl Denotation {
    pub(super) fn distribute_add(
        &mut self,
        scale: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.multiply_law_application(Law::DistributeAdd, &[scale, left, right])
    }
    /// `mul : Π(_ : Int). Π(_ : Int). Int` — the shared multiplication
    /// every open `IntegerMathTerm::Multiply` denotes, so the fixed
    /// order laws quantify over its applications.
    pub(super) fn multiply_operation(&mut self) -> Result<u32, BoundedDenotationError> {
        self.binary_integer_operation(|this| &mut this.multiplication.operation)
    }

    /// `mul l r` — the applicative denotation an open mathematical
    /// product keeps so the order laws and transport can name it.
    pub(super) fn multiply_terms(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.binary_terms(Self::multiply_operation, left, right)
    }

    /// `law a₁ … aₙ` — one multiplication-roster constant applied to the
    /// denoted endpoints and premise evidence, in statement order.
    fn multiply_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        self.roster_law_application(
            |this| &mut this.multiplication.laws,
            law,
            |this| -> Result<scheme_dsl::Syntax, BoundedDenotationError> {
                let integer = this.integer()?;
                let operation = this.multiply_operation()?;
                let less_or_equal = this.integer_less_or_equal()?;
                let zero = this.binary_integer(false, 0)?;
                let one = this.binary_integer(false, 1)?;
                let negative_one = this.binary_integer(true, 1)?;
                let constant = |position| scheme_at(position, Vec::new());
                let carrier = || constant(integer);
                let binders = |names: &[&'static str], body| {
                    pis(names.iter().map(|name| (*name, carrier())), body)
                };
                let le = |left, right| apps(constant(less_or_equal), [left, right]);
                let mul = |left, right| apps(constant(operation), [left, right]);
                let statement = match law {
                    Law::DistributeAdd => {
                        let addition = this.add_operation()?;
                        let add = |left, right| apps(constant(addition), [left, right]);
                        binders(
                            &["scale", "left", "right"],
                            id(
                                carrier(),
                                mul(v("scale"), add(v("left"), v("right"))),
                                add(mul(v("scale"), v("left")), mul(v("scale"), v("right"))),
                            ),
                        )
                    }
                    Law::MonotonePositive => binders(
                        &["a", "b", "r"],
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
                    Law::AntitoneNegative => binders(
                        &["a", "b", "r"],
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
                    Law::DivideLowerPositive(scalar_type)
                    | Law::DivideLowerNegative(scalar_type)
                    | Law::DivideUpperPositive(scalar_type)
                    | Law::DivideUpperNegative(scalar_type) => {
                        let divide =
                            this.integer_operation(IntegerOperation::ExactDivide(scalar_type))?;
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
                        binders(&["n", "d"], pi("_", dividend, pi("_", divisor, residual)))
                    }
                };
                Ok(statement)
            },
            arguments,
        )
    }

    /// `IntLe l' u'` evidence between two closed numerals: `refl` +
    /// `eq_le` when they agree, the binary-order strict laws plus
    /// `lt_le` when `l < u`. `None` when `l > u` — a defensive refusal,
    /// since callers derive the pair from an already-checked relation.
    pub(super) fn integer_le_numeral(
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
        if unbounded_integer(lower) >= unbounded_integer(upper) {
            return Ok(None);
        }
        let strict = self.literal_order(lower, upper)?;
        self.integer_law_application(
            IntegerLaw::LessThanToLessOrEqual,
            &[lower_term, upper_term, strict],
        )
        .map(Some)
    }

    /// `Id Int (mul l' r') p'` — the numeral-operation equation bridging
    /// a `mul` application over evaluated operands to their denoted
    /// product, the same exact interned assumption `numeral_sum` and
    /// `numeral_difference` name for their operations. Interned once per
    /// checked `(l, r, p)` value triple. `None` when `l · r ≠ p` — a
    /// defensive refusal, since callers derive the triple from a checked
    /// chain, so a miss keeps the instance fallback rather than naming a
    /// false equation.
    pub(super) fn numeral_product(
        &mut self,
        left: IntegerValue,
        right: IntegerValue,
        product: IntegerValue,
        left_term: TermHandle,
        right_term: TermHandle,
        product_term: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        self.numeral_binary_operation(
            left,
            right,
            product,
            left_term,
            right_term,
            product_term,
            |left, right| left.mul(right),
            Self::multiply_terms,
            |denotation| &mut denotation.multiplication.numeral_products,
        )
    }

    /// `Id Int (div_T n' d') q'` — the numeral-operation equation naming
    /// one truncating-division application over evaluated operands. `q`
    /// must be the exact toward-zero quotient `n / d` — recomputed with
    /// checked arithmetic, and `None` (keeping the instance fallback) on
    /// a zero divisor or any other value. The `div_T` constant is the
    /// per-type `ExactIntegerDivide` denotation the scalar term already
    /// carries, so the equation is exact for that machine operation.
    pub(super) fn numeral_quotient(
        &mut self,
        integer_type: IntegerType,
        dividend: IntegerValue,
        divisor: IntegerValue,
        quotient: IntegerValue,
        dividend_term: TermHandle,
        divisor_term: TermHandle,
        quotient_term: TermHandle,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let Some((exact, _)) = unbounded_integer(dividend).div_rem(&unbounded_integer(divisor))
        else {
            return Ok(None);
        };
        if exact != unbounded_integer(quotient) {
            return Ok(None);
        }
        let key = (integer_type, dividend, divisor, quotient);
        let position = self.interned_keyed_assumption(
            |this| &mut this.multiplication.numeral_quotients,
            key,
            |this| {
                let integer = this.integer_constant()?;
                let divide = this.integer_operation(IntegerOperation::ExactDivide(integer_type))?;
                let application = {
                    let function = this.constant(divide);
                    let function = this.arena.insert(Term::Apply {
                        function,
                        argument: dividend_term,
                    });
                    this.arena.insert(Term::Apply {
                        function,
                        argument: divisor_term,
                    })
                };
                Ok(this.arena.insert(Term::Id {
                    ty: integer,
                    left: application,
                    right: quotient_term,
                }))
            },
        )?;
        Ok(Some(self.constant(position)))
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
        let Some((lower, bound, product)) =
            math_less_or_equal_sides(conclusion, crate::term_matchers::is_math_multiply)
        else {
            return Ok(None);
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
        if super::math_literal_value(*bound_literal).map(unbounded_integer)
            != Some(unbounded_integer(endpoint_value))
        {
            return Ok(None);
        }
        let Proposition::Conjunction(parts) = premise else {
            return Ok(None);
        };
        let [sign_evidence, bound_evidence] = parts.as_slice() else {
            return Ok(None);
        };
        let one = ScalarTerm::integer(*scalar_type, scalar_type.one())
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
        let Some((expression, definition)) = affine_witness_root_expression(witness, definitions)
        else {
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
        let Some((endpoint_equality, endpoint_literal)) =
            super::affine_carrier_endpoint_equality(*scalar_type, lower, endpoint, definitions)
        else {
            return Ok(None);
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
        let Some(stepped) = self.affine_root_equality_step(
            lower == positive,
            witness,
            left_term,
            expression_term,
            definition,
            bound,
        )?
        else {
            return Ok(None);
        };
        bound = stepped;
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
        let endpoint_side = if let Some((equality, proposition)) = endpoint_equality {
            let literal_term = self.fixed_scalar_term(&endpoint_literal)?;
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
        if let Some((equality, proposition)) = endpoint_equality {
            let literal_term = self.fixed_scalar_term(&endpoint_literal)?;
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

impl Multiplication {
    /// Exact internally audited semantic roster, excluding arbitrary source,
    /// decision and value-parameter assumptions sharing the signature.
    pub(super) fn source_safety_roster(&self) -> Vec<u32> {
        self.operation
            .into_iter()
            .chain(self.laws.values().copied())
            .chain(self.numeral_products.values().copied())
            .chain(self.numeral_quotients.values().copied())
            .collect()
    }
}
