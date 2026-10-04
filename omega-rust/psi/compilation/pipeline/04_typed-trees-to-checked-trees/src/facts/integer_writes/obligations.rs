//! Concrete physical-width demands over one performed write. These private
//! predicates describe that occurrence; they are not authored call contracts.
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    CheckedScalarExpression as Scalar, PerformedIntegerWrite,
};
use numerics::bignum::BigInt;
use semantic_vocabulary::{IntegerType, IntegerValue};
use typed_trees::types::PrimitiveType;

/// Return the condition sufficient for this occurrence to fit its destination.
/// A missing condition conservatively requires the write to fit unconditionally;
/// a missing value is not evidence that the occurrence performs no write.
pub(crate) fn integer_write_obligation(
    write: &PerformedIntegerWrite,
    stored: IntegerType,
) -> Option<Boolean> {
    // Match the physical geometry admitted by IntegerAt, independently from
    // the larger mathematical integer vocabulary.
    if stored.is_address()
        || stored.bits() == 0
        || stored.bits() > 64
        || !stored.bits().is_multiple_of(8)
    {
        return None;
    }
    let primitive = write.primitive_type?;
    let semantic = crate::values::integer_type(primitive)?;
    if write
        .entry_value
        .as_ref()
        .is_some_and(|value| value.primitive_type() != Some(primitive))
    {
        return None;
    }
    let semantic_low = integer(semantic.minimum_value());
    let semantic_high = integer(semantic.maximum_value());
    let stored_low = integer(stored.minimum_value());
    let stored_high = integer(stored.maximum_value());
    let (low, high) = if let Some(range) = &write.integer_range {
        if range.minimum > range.maximum {
            return None;
        }
        (
            range.minimum.clone().max(semantic_low.clone()),
            range.maximum.clone().min(semantic_high.clone()),
        )
    } else {
        (semantic_low.clone(), semantic_high.clone())
    };
    if low > high {
        return None;
    }
    if low >= stored_low && high <= stored_high {
        return Some(Boolean::Constant(true));
    }
    let value = write.entry_value.as_ref()?;
    let fits = if stored_high < semantic_low || stored_low > semantic_high {
        Boolean::Constant(false)
    } else {
        let lower = if stored_low > semantic_low {
            Some(Boolean::IntegerComparison {
                kind: Comparison::LessOrEqual,
                left: Box::new(literal(&stored_low, primitive)?),
                right: Box::new(value.clone()),
            })
        } else {
            None
        };
        let upper = if stored_high < semantic_high {
            Some(Boolean::IntegerComparison {
                kind: Comparison::LessOrEqual,
                left: Box::new(value.clone()),
                right: Box::new(literal(&stored_high, primitive)?),
            })
        } else {
            None
        };
        match (lower, upper) {
            (Some(left), Some(right)) => Boolean::And {
                left: Box::new(left),
                right: Box::new(right),
            },
            (Some(bound), None) | (None, Some(bound)) => bound,
            (None, None) => Boolean::Constant(true),
        }
    };
    Some(match &write.entry_condition {
        None | Some(Boolean::Constant(true)) => fits,
        Some(condition) => Boolean::Or {
            left: Box::new(Boolean::Not(Box::new(condition.clone()))),
            right: Box::new(fits),
        },
    })
}

fn integer(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
    }
}

fn literal(value: &BigInt, primitive: PrimitiveType) -> Option<Scalar> {
    let text = value.to_string();
    let digits = text.strip_prefix('-').unwrap_or(&text);
    let literal = numerics::literals::IntegerLiteral::from_parts(
        text.starts_with('-'),
        numerics::literals::IntegerRadix::Decimal,
        digits,
    )
    .ok()?
    .with_landing(numerics::literals::IntegerLanding {
        landed_type: crate::values::landed_for_primitive(primitive)?,
        domain: numerics::arithmetic::ArithmeticDomain::Exact,
    });
    Some(Scalar::IntegerLiteral { literal })
}
