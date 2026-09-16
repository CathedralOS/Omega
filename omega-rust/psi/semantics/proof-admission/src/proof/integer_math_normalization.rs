//! The bridge between fixed-width scalar relations and mathematical
//! integer relations: lifting a relation over fixed-width terms into the
//! mathematical vocabulary, lowering the canonical carrier-bound shapes
//! back, and matching a retained proposition against a requested one
//! under either normalization.

use semantic_vocabulary::{IntegerMathTerm, IntegerValue, Proposition, ScalarTerm};

/// Canonically embed a relation over fixed-width value/literal terms into the
/// mathematical-integer relation vocabulary. Compound machine terms are not
/// silently reinterpreted by this bridge.
pub fn lift_fixed_integer_relation(proposition: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match proposition {
        Proposition::Equal(left, right) => (0, left, right),
        Proposition::LessThan(left, right) => (1, left, right),
        Proposition::LessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    let mut left = lift_fixed_integer_term(left)?;
    let mut right = lift_fixed_integer_term(right)?;
    Some(match kind {
        0 => {
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::IntegerMathEqual(left, right)
        }
        1 => Proposition::IntegerMathLessThan(left, right),
        _ => Proposition::IntegerMathLessOrEqual(left, right),
    })
}

/// Inverse of [`lift_fixed_integer_relation`] for the canonical carrier-bound
/// shapes used by exact-cast representability.
pub fn lower_integer_math_relation(proposition: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match proposition {
        Proposition::IntegerMathEqual(left, right) => (0, left, right),
        Proposition::IntegerMathLessThan(left, right) => (1, left, right),
        Proposition::IntegerMathLessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    let source_type = [left, right].into_iter().find_map(|term| match term {
        IntegerMathTerm::MathValue { source_type, .. } => Some(*source_type),
        _ => None,
    })?;
    let lower = |term: &IntegerMathTerm| match term {
        IntegerMathTerm::MathValue {
            source_type: actual,
            value,
        } if *actual == source_type => Some(ScalarTerm::value(
            *value,
            semantic_vocabulary::ScalarType::Integer(source_type),
        )),
        IntegerMathTerm::IntegerLiteral(literal) => {
            ScalarTerm::integer(source_type, literal.as_integer_value(source_type)?).ok()
        }
        _ => None,
    };
    let left = lower(left)?;
    let right = lower(right)?;
    Some(match kind {
        0 => Proposition::Equal(left, right),
        1 => Proposition::LessThan(left, right),
        _ => Proposition::LessOrEqual(left, right),
    })
}

/// The premise-citation relation every bounded checker route shares:
/// exact equality or the fixed-width↔mathematical carrier normalization
/// above. `pub(crate)` so the mathematical-core denotation bridge applies
/// the same relation instead of restating it.
pub(crate) fn propositions_match_under_integer_math_normalization(
    retained: &Proposition,
    requested: &Proposition,
) -> bool {
    retained == requested
        || lift_fixed_integer_relation(retained).as_ref() == Some(requested)
        || lower_integer_math_relation(retained).as_ref() == Some(requested)
}

fn lift_fixed_integer_term(term: &ScalarTerm) -> Option<IntegerMathTerm> {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: semantic_vocabulary::ScalarType::Integer(source_type),
        } if !source_type.is_address() => Some(IntegerMathTerm::MathValue {
            source_type: *source_type,
            value: *id,
        }),
        ScalarTerm::Integer { scalar_type, value } if !scalar_type.is_address() => {
            debug_assert!(matches!(
                value,
                IntegerValue::Signed(_) | IntegerValue::Unsigned(_)
            ));
            Some(IntegerMathTerm::literal(*value))
        }
        _ => None,
    }
}
