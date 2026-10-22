//! Source-free IEEE observations about a fact's place. Construction chooses
//! one format and lands its bound once; equality of these records includes
//! that format. The producer must establish the selected IEEE meaning and
//! truth of the observation. This value only describes it and answers weaker
//! consequences, never grants authority from an operator spelling.

use numerics::bignum::ExactFloat;
use numerics::literals::FloatFormat;
use std::cmp::Ordering;
use typed_trees::expression::BinaryOperator;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatPredicate {
    format: FloatFormat,
    comparison: BinaryOperator,
    // None denotes non-NaN, not an unspecified endpoint of an ordered range.
    bound_bits: Option<u64>,
}

impl FloatPredicate {
    pub fn non_nan(format: FloatFormat) -> Self {
        Self {
            format,
            comparison: BinaryOperator::Equal,
            bound_bits: None,
        }
    }

    /// Only positive equality and order exclude NaN. In particular `!=`
    /// and negated ordered comparisons cannot become bound observations.
    pub fn comparison(
        format: FloatFormat,
        comparison: BinaryOperator,
        bound: &ExactFloat,
    ) -> Option<Self> {
        use BinaryOperator::{Equal, Greater, GreaterOrEqual, Less, LessOrEqual};
        if !matches!(
            comparison,
            Equal | Greater | GreaterOrEqual | Less | LessOrEqual
        ) || bound.partial_cmp_value(bound).is_none()
        {
            return None;
        }
        let bound_bits = match format {
            FloatFormat::F32 => u64::from(bound.to_f32().to_bits()),
            FloatFormat::F64 => bound.to_f64().to_bits(),
        };
        Some(Self {
            format,
            comparison,
            bound_bits: Some(bound_bits),
        })
    }

    pub fn format(self) -> FloatFormat {
        self.format
    }

    /// The IEEE spelling of an exact equality bound.
    ///
    /// Signed zero keeps the sign bit in the spelling. Callers compare that
    /// spelling with IEEE equality, where the two zeros match and NaN matches
    /// nothing. A non-equality or a NaN exclusion has no single bound.
    pub fn equal_bound_spelling(self) -> Option<String> {
        if self.comparison != BinaryOperator::Equal {
            return None;
        }
        let bits = self.bound_bits?;
        let value = match self.format {
            FloatFormat::F32 => f64::from(f32::from_bits(bits as u32)),
            FloatFormat::F64 => f64::from_bits(bits),
        };
        Some(
            numerics::literals::FloatLiteral::from_f64(value)
                .text()
                .to_owned(),
        )
    }

    /// State joins retain only shared consequences. Bounds weaken toward
    /// existing endpoints; no new endpoint is generated during flow iteration.
    pub fn common_consequence(self, other: Self) -> Option<Self> {
        if self.implies(other) {
            Some(other)
        } else if other.implies(self) {
            Some(self)
        } else if self.format == other.format {
            Some(Self::non_nan(self.format))
        } else {
            None
        }
    }

    fn bound(self) -> Option<ExactFloat> {
        let bits = self.bound_bits?;
        Some(match self.format {
            FloatFormat::F32 => ExactFloat::from_f32(f32::from_bits(bits as u32)),
            FloatFormat::F64 => ExactFloat::from_f64(f64::from_bits(bits)),
        })
    }

    /// An observation about one format never proves a fact about another.
    /// Strictness stays in the comparison, not a host-float epsilon.
    pub fn implies(self, required: Self) -> bool {
        use BinaryOperator::{Equal, Greater, GreaterOrEqual, Less, LessOrEqual};
        if self.format != required.format {
            return false;
        }
        let Some(required_bound) = required.bound() else {
            return true;
        };
        let Some(bound) = self.bound() else {
            return false;
        };
        let Some(ordering) = bound.partial_cmp_value(&required_bound) else {
            return false;
        };
        match required.comparison {
            Equal => self.comparison == Equal && ordering == Ordering::Equal,
            Greater | GreaterOrEqual
                if matches!(self.comparison, Equal | Greater | GreaterOrEqual) =>
            {
                ordering == Ordering::Greater
                    || (ordering == Ordering::Equal
                        && (required.comparison == GreaterOrEqual || self.comparison == Greater))
            }
            Less | LessOrEqual if matches!(self.comparison, Equal | Less | LessOrEqual) => {
                ordering == Ordering::Less
                    || (ordering == Ordering::Equal
                        && (required.comparison == LessOrEqual || self.comparison == Less))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ieee_bound_implication_keeps_strictness_format_and_nan() {
        let predicate = |format, operation, value| {
            FloatPredicate::comparison(format, operation, &ExactFloat::from_f64(value)).unwrap()
        };
        let lower = predicate(FloatFormat::F64, BinaryOperator::GreaterOrEqual, 0.0);
        assert!(lower.implies(FloatPredicate::non_nan(FloatFormat::F64)));
        assert!(lower.implies(predicate(FloatFormat::F64, BinaryOperator::Greater, -1.0)));
        assert!(!lower.implies(predicate(FloatFormat::F64, BinaryOperator::Greater, 0.0)));
        assert!(!lower.implies(FloatPredicate::non_nan(FloatFormat::F32)));
        assert!(
            !predicate(FloatFormat::F32, BinaryOperator::GreaterOrEqual, 16777217.0).implies(
                predicate(FloatFormat::F32, BinaryOperator::Greater, 16777216.0)
            )
        );
        assert!(
            FloatPredicate::comparison(
                FloatFormat::F64,
                BinaryOperator::Equal,
                &ExactFloat::from_f64(f64::NAN)
            )
            .is_none()
        );
        assert!(
            FloatPredicate::comparison(
                FloatFormat::F64,
                BinaryOperator::NotEqual,
                &ExactFloat::from_f64(0.0)
            )
            .is_none()
        );
    }
}
