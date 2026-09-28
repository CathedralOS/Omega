//! Open-term ring normalization for the mathematical integer vocabulary.
//!
//! `ClosedIntegerEvaluator` decides a relation only while both operands stay
//! closed. Exact `Add`/`Subtract`/`Multiply` terms additionally obey the
//! commutative-ring identities, so `(acc + 1) + (remaining - 1)` and
//! `acc + remaining` share one normal form over the open atoms `acc` and
//! `remaining`. This module owns that normal form — the licensed arithmetic
//! derivation `PrimitiveJudgment::ClosedIntegerRelation` consults once
//! closed evaluation leaves a term open.
//!
//! It decides identities only: `left - right` reducing to a closed constant
//! `c` establishes `left cmp right` as `c cmp 0` for every integer valuation
//! of the atoms. A nonconstant difference, an uninterpreted operation or a
//! resource refusal establishes nothing, so the judgment never invents a
//! per-instance axiom and never reads a wrapping machine operation as exact
//! arithmetic: only the `IntegerMathTerm` ring vocabulary participates, and
//! fixed-width terms enter it through the existing lifting bridge.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use numerics::bignum::BigInt;
use semantic_vocabulary::IntegerMathTerm;

use super::closed_integer::{
    ClosedIntegerEvaluationError, ClosedIntegerEvaluator, big_integer_literal,
    check_integer_math_term_size,
};

/// Monomials retained per invocation — a resource refusal, never a judgment.
/// Distributing `Multiply` over two `n`-term sums needs `n²` monomials, so
/// the cap bounds expansion work as well as the retained form.
const MAXIMUM_MONOMIALS: usize = 4_096;

/// One product of open atoms: `atom → exponent`. The empty map is the
/// constant monomial; atoms are the subterms normalization deliberately
/// does not interpret — `MathValue` leaves, open shifts and any other
/// non-ring form — the same identity the denotation interner assigns them.
type Monomial = BTreeMap<IntegerMathTerm, u32>;

/// `Σ coefficient·monomial` — the canonical form an `IntegerMathTerm`
/// reduces to. Zero coefficients are dropped on every merge, so the map is
/// canonical up to atom identity.
type NormalForm = BTreeMap<Monomial, BigInt>;

fn constant(value: BigInt) -> NormalForm {
    if value.is_zero() {
        return NormalForm::new();
    }
    let mut form = NormalForm::new();
    form.insert(Monomial::new(), value);
    form
}

fn atom(term: &IntegerMathTerm) -> NormalForm {
    let mut monomial = Monomial::new();
    monomial.insert(term.clone(), 1);
    let mut form = NormalForm::new();
    form.insert(monomial, BigInt::from_u128(1));
    form
}

/// Invocation-owned budget for one normalization pass.
#[derive(Default)]
struct Normalizer {
    monomials: usize,
}

impl Normalizer {
    fn charge(&mut self, monomials: usize) -> Result<(), ClosedIntegerEvaluationError> {
        self.monomials = self
            .monomials
            .checked_add(monomials)
            .filter(|total| *total <= MAXIMUM_MONOMIALS)
            .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
        Ok(())
    }

    /// `difference += sign · added`, dropping zero coefficients so the
    /// result stays canonical.
    fn merge(
        &mut self,
        difference: &mut NormalForm,
        added: &NormalForm,
        negate: bool,
    ) -> Result<(), ClosedIntegerEvaluationError> {
        self.charge(added.len())?;
        for (monomial, coefficient) in added {
            let coefficient = if negate {
                coefficient.negate()
            } else {
                coefficient.clone()
            };
            let merged = difference
                .entry(monomial.clone())
                .or_insert_with(BigInt::zero)
                .add(&coefficient);
            if merged.is_zero() {
                difference.remove(monomial);
            } else {
                difference.insert(monomial.clone(), merged);
            }
        }
        Ok(())
    }

    /// `left · right` under the product rule: monomials merge by atomwise
    /// exponent addition and coefficients multiply exactly.
    fn product(
        &mut self,
        left: &NormalForm,
        right: &NormalForm,
    ) -> Result<NormalForm, ClosedIntegerEvaluationError> {
        self.charge(
            left.len()
                .checked_mul(right.len())
                .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?,
        )?;
        let mut result = NormalForm::new();
        for (left_monomial, left_coefficient) in left {
            for (right_monomial, right_coefficient) in right {
                let mut monomial = left_monomial.clone();
                for (atom, exponent) in right_monomial {
                    let exponent = monomial
                        .get(atom)
                        .copied()
                        .unwrap_or(0)
                        .checked_add(*exponent)
                        .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
                    monomial.insert(atom.clone(), exponent);
                }
                let coefficient = left_coefficient.mul(right_coefficient);
                if coefficient.is_zero() {
                    continue;
                }
                let merged = result
                    .entry(monomial.clone())
                    .or_insert_with(BigInt::zero)
                    .add(&coefficient);
                if merged.is_zero() {
                    result.remove(&monomial);
                } else {
                    result.insert(monomial, merged);
                }
            }
        }
        Ok(result)
    }

    /// The term's canonical form. `Add`/`Subtract`/`Multiply` participate in
    /// the ring; every other node first tries closed evaluation so a closed
    /// subtree contributes its exact value, then falls back to one opaque
    /// atom for open leaves, open shifts and non-ring forms.
    fn normalize(
        &mut self,
        term: &IntegerMathTerm,
    ) -> Result<NormalForm, ClosedIntegerEvaluationError> {
        match term {
            IntegerMathTerm::IntegerLiteral(literal) => Ok(constant(big_integer_literal(*literal))),
            IntegerMathTerm::Add(left, right) => {
                let left = self.normalize(left)?;
                let right = self.normalize(right)?;
                let mut sum = left;
                self.merge(&mut sum, &right, false)?;
                Ok(sum)
            }
            IntegerMathTerm::Subtract(left, right) => {
                let left = self.normalize(left)?;
                let right = self.normalize(right)?;
                let mut difference = left;
                self.merge(&mut difference, &right, true)?;
                Ok(difference)
            }
            IntegerMathTerm::Multiply(left, right) => {
                let left = self.normalize(left)?;
                let right = self.normalize(right)?;
                self.product(&left, &right)
            }
            _ => match ClosedIntegerEvaluator::default().evaluate_closed(term)? {
                Some(value) => Ok(constant(value)),
                None => Ok(atom(term)),
            },
        }
    }
}

/// The exact value of `left - right` when the open parts cancel — `Some`
/// only for a constant difference, so `left cmp right` is that constant's
/// ordering against zero under every valuation of the open atoms.
///
/// The shared mathematical integer term size preflight runs first; the
/// normalizer's own monomial budget then refuses expansions too large to
/// keep. Both failures are resource refusals, never judgments.
pub(crate) fn constant_difference(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Result<Option<BigInt>, ClosedIntegerEvaluationError> {
    check_integer_math_term_size(left, right)?;
    let mut normalizer = Normalizer::default();
    let left = normalizer.normalize(left)?;
    let right = normalizer.normalize(right)?;
    let mut difference = left;
    normalizer.merge(&mut difference, &right, true)?;
    Ok(match difference.len() {
        0 => Some(BigInt::zero()),
        1 => difference
            .first_key_value()
            .and_then(|(monomial, coefficient)| monomial.is_empty().then(|| coefficient.clone())),
        _ => None,
    })
}

/// `left cmp right` when their difference normalizes to a constant.
pub(crate) fn constant_difference_ordering(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Result<Option<Ordering>, ClosedIntegerEvaluationError> {
    Ok(constant_difference(left, right)?.map(|difference| difference.cmp(&BigInt::zero())))
}

#[cfg(test)]
mod tests {
    use super::{constant_difference, constant_difference_ordering};
    use numerics::bignum::BigInt;
    use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ValueId};
    use std::cmp::Ordering;

    fn math_type() -> IntegerType {
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
    }

    fn atom(id: u64) -> IntegerMathTerm {
        IntegerMathTerm::MathValue {
            source_type: math_type(),
            value: ValueId::new(id).expect("value identity"),
        }
    }

    fn integer(value: i128) -> IntegerMathTerm {
        IntegerMathTerm::literal(IntegerValue::Signed(value))
    }

    fn add(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
        IntegerMathTerm::Add(Box::new(left), Box::new(right))
    }

    fn subtract(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
        IntegerMathTerm::Subtract(Box::new(left), Box::new(right))
    }

    fn multiply(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
        IntegerMathTerm::Multiply(Box::new(left), Box::new(right))
    }

    #[test]
    fn accumulator_recurrence_normalizes_to_zero_difference() {
        // (acc + 1) + (remaining - 1) - (acc + remaining) ≡ 0 — the update
        // preservation the cyclic-header check needs the kernel to decide.
        let acc = atom(1);
        let remaining = atom(2);
        let updated = add(
            add(acc.clone(), integer(1)),
            subtract(remaining.clone(), integer(1)),
        );
        let conserved = add(acc, remaining);
        assert_eq!(
            constant_difference_ordering(&updated, &conserved),
            Ok(Some(Ordering::Equal))
        );
    }

    #[test]
    fn commutativity_and_distribution_are_identities_not_assumptions() {
        let x = atom(1);
        let y = atom(2);
        assert_eq!(
            constant_difference(
                &multiply(x.clone(), y.clone()),
                &multiply(y.clone(), x.clone())
            ),
            Ok(Some(BigInt::zero())),
            "x·y and y·x normalize identically"
        );
        let distributed = multiply(x.clone(), add(y.clone(), integer(3)));
        let expanded = add(multiply(x.clone(), y), multiply(x, integer(3)));
        assert_eq!(
            constant_difference_ordering(&distributed, &expanded),
            Ok(Some(Ordering::Equal))
        );
    }

    #[test]
    fn constant_offsets_carry_their_exact_ordering() {
        let x = atom(1);
        assert_eq!(
            constant_difference_ordering(&x.clone(), &add(x.clone(), integer(1))),
            Ok(Some(Ordering::Less))
        );
        assert_eq!(
            constant_difference_ordering(&add(x.clone(), integer(1)), &subtract(x, integer(-2))),
            Ok(Some(Ordering::Less))
        );
    }

    #[test]
    fn nonconstant_differences_and_unmatched_atoms_establish_nothing() {
        let x = atom(1);
        let y = atom(2);
        // x + 1 - y has a surviving atom — no relation is decided.
        assert_eq!(
            constant_difference(&add(x.clone(), integer(1)), &y),
            Ok(None)
        );
        // A wrong update (acc forwarded unchanged) is not silently equal:
        // (acc) + (remaining - 1) - (acc + remaining) = -1 is constant, so
        // the comparison IS decided — as strictly less, not equal.
        let acc = atom(3);
        let remaining = atom(4);
        let forwarded = add(acc.clone(), subtract(remaining.clone(), integer(1)));
        let conserved = add(acc, remaining);
        assert_eq!(
            constant_difference_ordering(&forwarded, &conserved),
            Ok(Some(Ordering::Less)),
            "the wrong-step difference is decided as <, never as ="
        );
        // Distinct atoms stay distinct.
        assert_eq!(constant_difference(&x, &y), Ok(None));
    }

    #[test]
    fn open_operations_outside_the_ring_stay_opaque() {
        // An open shift is an atom: it cancels against itself only.
        let shift = IntegerMathTerm::ShiftLeft {
            value: Box::new(atom(1)),
            count: Box::new(integer(3)),
        };
        let x = atom(1);
        assert_eq!(
            constant_difference(&shift, &multiply(x, integer(8))),
            Ok(None),
            "shift(x,3) is not secretly rewritten to 8x"
        );
        assert_eq!(
            constant_difference_ordering(&shift, &shift),
            Ok(Some(Ordering::Equal))
        );
        // A closed shift folds to its exact value.
        let closed = IntegerMathTerm::ShiftLeft {
            value: Box::new(integer(1)),
            count: Box::new(integer(5)),
        };
        assert_eq!(
            constant_difference(&closed, &integer(32)),
            Ok(Some(BigInt::zero()))
        );
    }
}
