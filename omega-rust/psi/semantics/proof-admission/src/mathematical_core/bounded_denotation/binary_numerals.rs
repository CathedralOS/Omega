//! Fixed-width literal meaning for discrete order certificates.
//!
//! `zero`, `double`, `odd` and `negate` denote 0, 2x, 2x+1 and -x in
//! the existing abstract `Int` interface. Five fixed arithmetic assumptions
//! below suffice to derive each adjacent-literal strict order. The kernel
//! checks the definitions, law applications and endpoint transports; no
//! literal pair contributes an assumed order or equality statement.
//!
//! Prefix definitions share `(constructor, preceding declaration)` rather
//! than retaining a BigInt for every prefix. Their bodies are shallow, and
//! the largest fixed scalar magnitude has 128 digits. Larger evaluated
//! integers retain the parent's exact-value opaque interning: this slice
//! does not change their accepted range or claim an open arithmetic model.

use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};

use super::super::scheme_dsl::{self, Syntax, apps, id, pi, scheme_at, v};
use super::{BoundedDenotationError, Declaration, Denotation, IntegerLaw, Term, TermHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Constructor {
    Double,
    Odd,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Law {
    DoubleZero,
    NegateZero,
    EvenBeforeOdd,
    OddBeforeNextEven,
    NegateOrder,
}

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct BinaryNumerals {
    zero: Option<u32>,
    constructors: BTreeMap<Constructor, u32>,
    prefixes: BTreeMap<(Constructor, u32), u32>,
    laws: BTreeMap<Law, u32>,
}

/// Exactly the magnitude range a fixed scalar literal can carry, including
/// i128::MIN's unsigned magnitude. Other closed values keep their existing
/// opaque representation, so even a 65,536-bit reflexive equality stays cheap.
pub(super) fn fixed_magnitude(value: &BigInt) -> Option<(bool, u128)> {
    if value.bit_length() > u128::BITS as usize {
        return None;
    }
    let radix = BigInt::from_u128(1_u128 << 64);
    let (high, low) = value.abs().div_rem(&radix)?;
    Some((
        value.is_negative(),
        (u128::from(high.to_u64()?) << 64) | u128::from(low.to_u64()?),
    ))
}

impl Denotation {
    fn numeral_zero(&mut self) -> Result<u32, BoundedDenotationError> {
        if let Some(position) = self.binary_numerals.zero {
            return Ok(position);
        }
        let integer = self.integer_constant()?;
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, integer));
        self.binary_numerals.zero = Some(position);
        Ok(position)
    }

    fn numeral_constructor(
        &mut self,
        constructor: Constructor,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(&position) = self.binary_numerals.constructors.get(&constructor) {
            return Ok(position);
        }
        let integer = self.integer_constant()?;
        let ty = self.arena.insert(Term::Pi {
            domain: integer,
            codomain: integer,
        });
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.binary_numerals
            .constructors
            .insert(constructor, position);
        Ok(position)
    }

    fn numeral_prefix(
        &mut self,
        constructor: Constructor,
        prefix: u32,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(&position) = self.binary_numerals.prefixes.get(&(constructor, prefix)) {
            return Ok(position);
        }
        let function_position = self.numeral_constructor(constructor)?;
        let function = self.constant(function_position);
        let argument = self.constant(prefix);
        let body = self.arena.insert(Term::Apply { function, argument });
        let integer = self.integer_constant()?;
        let position = self.position()?;
        self.declarations
            .push(Declaration::definition(0, integer, body));
        self.binary_numerals
            .prefixes
            .insert((constructor, prefix), position);
        Ok(position)
    }

    pub(super) fn binary_integer(
        &mut self,
        negative: bool,
        magnitude: u128,
    ) -> Result<u32, BoundedDenotationError> {
        let mut position = self.numeral_zero()?;
        for bit in (0..(u128::BITS - magnitude.leading_zeros())).rev() {
            let constructor = if magnitude & (1_u128 << bit) == 0 {
                Constructor::Double
            } else {
                Constructor::Odd
            };
            position = self.numeral_prefix(constructor, position)?;
        }
        if negative && magnitude != 0 {
            position = self.numeral_prefix(Constructor::Negate, position)?;
        }
        Ok(position)
    }

    fn binary_term(
        &mut self,
        negative: bool,
        magnitude: u128,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.binary_integer(negative, magnitude)?;
        Ok(self.constant(position))
    }

    fn numeral_law(&mut self, law: Law) -> Result<u32, BoundedDenotationError> {
        if let Some(&position) = self.binary_numerals.laws.get(&law) {
            return Ok(position);
        }
        let integer = self.integer()?;
        let less_than = self.integer_less_than()?;
        let zero = self.numeral_zero()?;
        let double = self.numeral_constructor(Constructor::Double)?;
        let odd = self.numeral_constructor(Constructor::Odd)?;
        let negate = self.numeral_constructor(Constructor::Negate)?;
        let constant = |position| scheme_at(position, Vec::new());
        let apply = |position, value: Syntax| apps(constant(position), [value]);
        let lt = |left, right| apps(constant(less_than), [left, right]);
        let carrier = || constant(integer);
        let statement = match law {
            Law::DoubleZero => id(carrier(), apply(double, constant(zero)), constant(zero)),
            Law::NegateZero => id(carrier(), apply(negate, constant(zero)), constant(zero)),
            Law::EvenBeforeOdd => pi(
                "x",
                carrier(),
                lt(apply(double, v("x")), apply(odd, v("x"))),
            ),
            Law::OddBeforeNextEven => pi(
                "x",
                carrier(),
                pi(
                    "y",
                    carrier(),
                    pi(
                        "_",
                        lt(v("x"), v("y")),
                        lt(apply(odd, v("x")), apply(double, v("y"))),
                    ),
                ),
            ),
            Law::NegateOrder => pi(
                "x",
                carrier(),
                pi(
                    "y",
                    carrier(),
                    pi(
                        "_",
                        lt(v("x"), v("y")),
                        lt(apply(negate, v("y")), apply(negate, v("x"))),
                    ),
                ),
            ),
        };
        let ty = scheme_dsl::build(&mut self.arena, &mut Vec::new(), &statement);
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.binary_numerals.laws.insert(law, position);
        Ok(position)
    }

    fn numeral_law_application(
        &mut self,
        law: Law,
        arguments: &[TermHandle],
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.numeral_law(law)?;
        let mut function = self.constant(position);
        for &argument in arguments {
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    /// Derive n < n+1 by the binary carry chain. Only an odd low digit
    /// recurses, so there are at most 128 carry steps, not n successors.
    /// Rebuilding the canonical prefix handles costs quadratic map visits
    /// in the digit count; the retained definitions remain linear.
    fn positive_adjacent(&mut self, lower: u128) -> Result<TermHandle, BoundedDenotationError> {
        let half = self.binary_term(false, lower / 2)?;
        if lower & 1 != 0 {
            let evidence = self.positive_adjacent(lower / 2)?;
            let next_half = self.binary_term(false, lower / 2 + 1)?;
            return self
                .numeral_law_application(Law::OddBeforeNextEven, &[half, next_half, evidence]);
        }
        let evidence = self.numeral_law_application(Law::EvenBeforeOdd, &[half])?;
        if lower != 0 {
            return Ok(evidence);
        }
        let zero_position = self.numeral_zero()?;
        let doubled_position = self.numeral_prefix(Constructor::Double, zero_position)?;
        let doubled = self.constant(doubled_position);
        let one = self.binary_term(false, 1)?;
        let equality = self.numeral_law_application(Law::DoubleZero, &[])?;
        self.integer_law_application(
            IntegerLaw::LessThanSubstituteLeft,
            &[doubled, half, one, equality, evidence],
        )
    }

    fn literal_adjacent(
        &mut self,
        lower: &ScalarTerm,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (_, value) = lower
            .integer_value()
            .ok_or(BoundedDenotationError::Unsupported(
                "discreteness needs a fixed literal",
            ))?;
        let (negative, magnitude) = match value {
            IntegerValue::Signed(value) => (value < 0, value.unsigned_abs()),
            IntegerValue::Unsigned(value) => (false, value),
        };
        if !negative {
            return self.positive_adjacent(magnitude);
        }
        let evidence = self.positive_adjacent(magnitude - 1)?;
        let left = self.binary_term(false, magnitude - 1)?;
        let right = self.binary_term(false, magnitude)?;
        let evidence = self.numeral_law_application(Law::NegateOrder, &[left, right, evidence])?;
        if magnitude != 1 {
            return Ok(evidence);
        }
        let zero_position = self.numeral_zero()?;
        let negative_zero_position = self.numeral_prefix(Constructor::Negate, zero_position)?;
        let negative_zero = self.constant(negative_zero_position);
        let negative_one = self.binary_term(true, 1)?;
        let equality = self.numeral_law_application(Law::NegateZero, &[])?;
        self.integer_law_application(
            IntegerLaw::LessThanSubstituteRight,
            &[negative_one, negative_zero, left, equality, evidence],
        )
    }

    /// Compare arbitrary unsigned fixed-width numerals by their binary
    /// prefixes. There are at most 128 recursive comparisons; shared prefix
    /// lookup has the same quadratic digit cost as the adjacent carry proof.
    fn positive_order(
        &mut self,
        lower: u128,
        upper: u128,
    ) -> Result<TermHandle, BoundedDenotationError> {
        debug_assert!(lower < upper);
        let lower_half = self.binary_term(false, lower / 2)?;
        let upper_half = self.binary_term(false, upper / 2)?;
        let evidence = if lower / 2 == upper / 2 {
            self.numeral_law_application(Law::EvenBeforeOdd, &[lower_half])?
        } else {
            let prefix_order = self.positive_order(lower / 2, upper / 2)?;
            let mut evidence = self.numeral_law_application(
                Law::OddBeforeNextEven,
                &[lower_half, upper_half, prefix_order],
            )?;
            let lower_odd = self.binary_term(false, lower / 2 * 2 + 1)?;
            let upper_even = self.binary_term(false, upper / 2 * 2)?;
            if lower & 1 == 0 {
                let prefix = self.binary_integer(false, lower / 2)?;
                let doubled = self.numeral_prefix(Constructor::Double, prefix)?;
                let doubled = self.constant(doubled);
                let first = self.numeral_law_application(Law::EvenBeforeOdd, &[lower_half])?;
                evidence = self.integer_law_application(
                    IntegerLaw::LessThanTransitivity,
                    &[doubled, lower_odd, upper_even, first, evidence],
                )?;
            }
            if upper & 1 != 0 {
                let last = self.numeral_law_application(Law::EvenBeforeOdd, &[upper_half])?;
                // When lower is zero this intermediate proof still starts
                // at double(zero); the outer transport below normalizes it.
                let lower_position = self.binary_integer(false, lower / 2)?;
                let lower_position = self.numeral_prefix(
                    if lower & 1 == 0 {
                        Constructor::Double
                    } else {
                        Constructor::Odd
                    },
                    lower_position,
                )?;
                let lower_raw = self.constant(lower_position);
                let upper_term = self.binary_term(false, upper)?;
                evidence = self.integer_law_application(
                    IntegerLaw::LessThanTransitivity,
                    &[lower_raw, upper_even, upper_term, evidence, last],
                )?;
            }
            evidence
        };
        if lower != 0 {
            return Ok(evidence);
        }
        let zero = self.numeral_zero()?;
        let doubled = self.numeral_prefix(Constructor::Double, zero)?;
        let doubled = self.constant(doubled);
        let zero = self.constant(zero);
        let upper = self.binary_term(false, upper)?;
        let equality = self.numeral_law_application(Law::DoubleZero, &[])?;
        self.integer_law_application(
            IntegerLaw::LessThanSubstituteLeft,
            &[doubled, zero, upper, equality, evidence],
        )
    }

    pub(super) fn literal_order(
        &mut self,
        lower: IntegerValue,
        upper: IntegerValue,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let parts = |value| match value {
            IntegerValue::Signed(value) => (value < 0, value.unsigned_abs()),
            IntegerValue::Unsigned(value) => (false, value),
        };
        let (negative_lower, lower) = parts(lower);
        let (negative_upper, upper) = parts(upper);
        if !negative_lower {
            debug_assert!(!negative_upper && lower < upper);
            return self.positive_order(lower, upper);
        }
        if negative_upper || upper == 0 {
            let order = self.positive_order(upper, lower)?;
            let positive_left = self.binary_term(false, upper)?;
            let positive_right = self.binary_term(false, lower)?;
            let order = self.numeral_law_application(
                Law::NegateOrder,
                &[positive_left, positive_right, order],
            )?;
            if upper != 0 {
                return Ok(order);
            }
            let zero = self.numeral_zero()?;
            let negative_zero = self.numeral_prefix(Constructor::Negate, zero)?;
            let negative_zero = self.constant(negative_zero);
            let left = self.binary_term(true, lower)?;
            let equality = self.numeral_law_application(Law::NegateZero, &[])?;
            return self.integer_law_application(
                IntegerLaw::LessThanSubstituteRight,
                &[left, negative_zero, positive_left, equality, order],
            );
        }
        // Negative to positive crosses canonical zero once.
        let left_value = if lower == 1_u128 << 127 {
            i128::MIN
        } else {
            -(lower as i128)
        };
        let first =
            self.literal_order(IntegerValue::Signed(left_value), IntegerValue::Signed(0))?;
        let second = self.positive_order(0, upper)?;
        let left = self.binary_term(true, lower)?;
        let zero = self.binary_term(false, 0)?;
        let right = self.binary_term(false, upper)?;
        self.integer_law_application(
            IntegerLaw::LessThanTransitivity,
            &[left, zero, right, first, second],
        )
    }

    pub(super) fn discreteness_evidence(
        &mut self,
        premise: &Proposition,
        evidence: TermHandle,
        goal: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let (
            Proposition::LessOrEqual(left, right),
            Proposition::LessThan(strict_left, strict_right),
        ) = (premise, goal)
        else {
            return Err(BoundedDenotationError::Unsupported(
                "checked discreteness has integer order endpoints",
            ));
        };
        let premise_type = self.denote(premise)?;
        let goal_type = self.denote(goal)?;
        let (
            Some(super::IntegerRelation::LessOrEqual { left: a, right: b }),
            Some(super::IntegerRelation::LessThan { left: c, right: d }),
        ) = (
            self.integer_relation(premise_type),
            self.integer_relation(goal_type),
        )
        else {
            return Err(BoundedDenotationError::Unsupported(
                "checked discreteness denotes in Int",
            ));
        };
        if right == strict_right {
            let adjacent = self.literal_adjacent(strict_left)?;
            self.integer_law_application(
                IntegerLaw::LessThanLessOrEqualTransitivity,
                &[c, a, b, adjacent, evidence],
            )
        } else {
            debug_assert_eq!(left, strict_left);
            let adjacent = self.literal_adjacent(right)?;
            self.integer_law_application(
                IntegerLaw::LessOrEqualLessThanTransitivity,
                &[a, b, d, evidence, adjacent],
            )
        }
    }
}
