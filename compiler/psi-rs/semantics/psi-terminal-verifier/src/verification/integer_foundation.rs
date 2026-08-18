//! Shared checked integer carriers and interval arithmetic for proof replay.

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm};

pub(super) fn canonical_conjunction(mut conjuncts: Vec<Proposition>) -> Proposition {
    match conjuncts.len() {
        0 => Proposition::Truth,
        1 => conjuncts.pop().expect("one conjunct exists"),
        _ => Proposition::Conjunction(conjuncts),
    }
}

pub(super) fn integer_value_cmp(left: IntegerValue, right: IntegerValue) -> std::cmp::Ordering {
    match (left, right) {
        (IntegerValue::Signed(left), IntegerValue::Signed(right)) => left.cmp(&right),
        (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => left.cmp(&right),
        (IntegerValue::Signed(left), IntegerValue::Unsigned(right)) => {
            if left < 0 {
                std::cmp::Ordering::Less
            } else {
                (left as u128).cmp(&right)
            }
        }
        (IntegerValue::Unsigned(left), IntegerValue::Signed(right)) => {
            if right < 0 {
                std::cmp::Ordering::Greater
            } else {
                left.cmp(&(right as u128))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerOffset {
    Nonnegative(u128),
    Negative(u128),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactIntegerOffsetOperation {
    Add,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactIntegerAffineOperation {
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactIntegerDivideRemainderTransfer {
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactIntegerIntervalPreimage {
    Interval((i128, i128)),
    Empty,
}

impl IntegerOffset {
    pub(super) fn from_value(value: IntegerValue) -> Self {
        match value {
            IntegerValue::Unsigned(value) => Self::Nonnegative(value),
            IntegerValue::Signed(value) if value < 0 => Self::Negative(value.unsigned_abs()),
            IntegerValue::Signed(value) => Self::Nonnegative(value as u128),
        }
    }

    pub(super) fn from_subtrahend(value: IntegerValue) -> Self {
        match Self::from_value(value) {
            Self::Nonnegative(value) => Self::Negative(value),
            Self::Negative(value) => Self::Nonnegative(value),
        }
    }

    pub(super) fn checked_add(self, right: Self) -> Option<Self> {
        match (self, right) {
            (Self::Nonnegative(left), Self::Nonnegative(right)) => {
                left.checked_add(right).map(Self::Nonnegative)
            }
            (Self::Negative(left), Self::Negative(right)) => {
                left.checked_add(right).map(Self::Negative)
            }
            (Self::Nonnegative(left), Self::Negative(right)) => Some(if left >= right {
                Self::Nonnegative(left - right)
            } else {
                Self::Negative(right - left)
            }),
            (Self::Negative(left), Self::Nonnegative(right)) => Some(if right >= left {
                Self::Nonnegative(right - left)
            } else {
                Self::Negative(left - right)
            }),
        }
    }

    pub(super) fn checked_multiply(self, factor: u128) -> Option<Self> {
        if factor == 0 {
            return Some(Self::Nonnegative(0));
        }
        match self {
            Self::Nonnegative(value) => value.checked_mul(factor).map(Self::Nonnegative),
            Self::Negative(value) => value.checked_mul(factor).map(Self::Negative),
        }
    }

    pub(super) fn checked_multiply_value(self, factor: IntegerValue) -> Option<Self> {
        let factor = Self::from_value(factor);
        self.checked_multiply_offset(factor)
    }

    pub(super) fn checked_multiply_offset(self, factor: Self) -> Option<Self> {
        let product = self.checked_multiply(factor.magnitude())?;
        Some(match factor {
            Self::Negative(_) => product.negated(),
            Self::Nonnegative(_) => product,
        })
    }

    pub(super) const fn negated(self) -> Self {
        match self {
            Self::Nonnegative(0) | Self::Negative(0) => Self::Nonnegative(0),
            Self::Nonnegative(value) => Self::Negative(value),
            Self::Negative(value) => Self::Nonnegative(value),
        }
    }

    pub(super) const fn magnitude(self) -> u128 {
        match self {
            Self::Nonnegative(value) | Self::Negative(value) => value,
        }
    }

    pub(super) fn is_representable(self, integer_type: psi_core::IntegerType) -> bool {
        match (integer_type.sign(), self) {
            (IntegerSign::Unsigned, Self::Nonnegative(value)) => {
                integer_type.admits(IntegerValue::Unsigned(value))
            }
            (IntegerSign::Unsigned, Self::Negative(_)) => false,
            (IntegerSign::Signed, Self::Nonnegative(value)) => i128::try_from(value)
                .ok()
                .is_some_and(|value| integer_type.admits(IntegerValue::Signed(value))),
            (IntegerSign::Signed, Self::Negative(value)) => signed_negative_magnitude(value)
                .is_some_and(|value| integer_type.admits(IntegerValue::Signed(value))),
        }
    }
}

pub(super) fn checked_signed_integer_product(
    product: Option<IntegerOffset>,
    factor: IntegerValue,
) -> Option<IntegerOffset> {
    if IntegerOffset::from_value(factor).magnitude() == 0 {
        return Some(IntegerOffset::Nonnegative(0));
    }
    product?.checked_multiply_value(factor)
}

pub(super) fn exact_integer_carrier_total_hull_obligation(
    hull: (i128, i128),
    interval: (i128, i128),
) -> Option<Proposition> {
    if hull.0 >= interval.0 && hull.1 <= interval.1 {
        Some(Proposition::Truth)
    } else if hull.1 < interval.0 || hull.0 > interval.1 {
        Some(Proposition::Falsehood)
    } else {
        None
    }
}

pub(super) fn fixed_integer_type_interval(
    integer_type: psi_core::IntegerType,
) -> Option<(i128, i128)> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    Some((
        fixed_integer_value(integer_type, integer_type.minimum_value())?,
        fixed_integer_value(integer_type, integer_type.maximum_value())?,
    ))
}

pub(super) fn fixed_integer_value(
    integer_type: psi_core::IntegerType,
    value: IntegerValue,
) -> Option<i128> {
    match (integer_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(value),
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => i128::try_from(value).ok(),
        _ => None,
    }
}

pub(super) fn checked_integer_floor_division(dividend: i128, divisor: i128) -> Option<i128> {
    let quotient = dividend.checked_div(divisor)?;
    let remainder = dividend.checked_rem(divisor)?;
    Some(if remainder != 0 && (remainder < 0) != (divisor < 0) {
        quotient.checked_sub(1)?
    } else {
        quotient
    })
}

pub(super) fn checked_integer_ceil_division(dividend: i128, divisor: i128) -> Option<i128> {
    let quotient = dividend.checked_div(divisor)?;
    let remainder = dividend.checked_rem(divisor)?;
    Some(if remainder != 0 && (remainder < 0) == (divisor < 0) {
        quotient.checked_add(1)?
    } else {
        quotient
    })
}

pub(super) fn exact_integer_source_interval_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    target_minimum: i128,
    target_maximum: i128,
) -> Proposition {
    let Some(root_minimum) = integer_value_as_i128(root_type.minimum_value()) else {
        return Proposition::Falsehood;
    };
    let Some(root_maximum) = integer_value_as_i128(root_type.maximum_value()) else {
        return Proposition::Falsehood;
    };
    if target_minimum > root_maximum || target_maximum < root_minimum {
        return Proposition::Falsehood;
    }

    let root_boundary = |boundary: i128| {
        let value = match root_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(boundary),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(boundary).ok()?),
        };
        ScalarTerm::integer(root_type, value).ok()
    };
    let mut bounds = Vec::with_capacity(2);
    if target_minimum > root_minimum {
        let Some(boundary) = root_boundary(target_minimum) else {
            return Proposition::Falsehood;
        };
        bounds.push(Proposition::LessOrEqual(boundary, root.clone()));
    }
    if target_maximum < root_maximum {
        let Some(boundary) = root_boundary(target_maximum) else {
            return Proposition::Falsehood;
        };
        bounds.push(Proposition::LessOrEqual(root, boundary));
    }
    match bounds.len() {
        0 => Proposition::Truth,
        1 => bounds
            .pop()
            .expect("one translated exact-cast bound exists"),
        _ => canonical_conjunction(bounds),
    }
}

pub(super) fn integer_value_as_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

pub(super) fn signed_negative_magnitude(magnitude: u128) -> Option<i128> {
    if magnitude == 1_u128 << 127 {
        Some(i128::MIN)
    } else {
        i128::try_from(magnitude).ok().and_then(i128::checked_neg)
    }
}

pub(super) fn integer_type_span(integer_type: psi_core::IntegerType) -> u128 {
    if integer_type.bits() == 128 {
        u128::MAX
    } else {
        (1_u128 << integer_type.bits()) - 1
    }
}

pub(super) fn landed_integer_constant_value(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    semantic_axioms: &[Proposition],
    prior_axiom_count: usize,
) -> Option<IntegerValue> {
    let (known_type, value) = term.integer_value().or_else(|| {
        semantic_axioms[..prior_axiom_count.min(semantic_axioms.len())]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == term => right.integer_value(),
                _ => None,
            })
    })?;
    (known_type == integer_type && integer_type.admits(value)).then_some(value)
}

pub(super) fn nonnegative_integer_factor(
    integer_type: psi_core::IntegerType,
    factor: IntegerValue,
) -> Option<u128> {
    match (integer_type.sign(), factor) {
        (IntegerSign::Unsigned, IntegerValue::Unsigned(factor)) => Some(factor),
        (IntegerSign::Signed, IntegerValue::Signed(factor)) => u128::try_from(factor).ok(),
        _ => None,
    }
}

pub(super) fn known_integer_term_value(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<IntegerValue> {
    let (known_type, value) = term.integer_value().or_else(|| {
        semantic_axioms.iter().rev().find_map(|axiom| {
            let Proposition::Equal(left, right) = axiom else {
                return None;
            };
            if left == term {
                right.integer_value()
            } else if right == term {
                left.integer_value()
            } else {
                None
            }
        })
    })?;
    (known_type == integer_type && integer_type.admits(value)).then_some(value)
}
