//! The bridge between fixed-width scalar relations and mathematical
//! integer relations: lifting a relation over fixed-width terms into the
//! mathematical vocabulary, lowering the canonical carrier-bound shapes
//! back, and matching a retained proposition against a requested one
//! under either normalization.

use semantic_vocabulary::{IntegerCarrier, IntegerMathTerm, IntegerType, Proposition, ScalarTerm};

/// Canonically embed a relation over fixed-width terms into the mathematical
/// vocabulary. Projections and compound operations retain their complete scalar
/// meaning inside `Embed`; no operation acquires mathematical arithmetic laws.
pub fn lift_fixed_integer_relation(proposition: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match proposition {
        Proposition::Equal(left, right) => (0, left, right),
        Proposition::LessThan(left, right) => (1, left, right),
        Proposition::LessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    crate::check_predicate_evaluation_size([proposition]).ok()?;
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

/// Inverse of [`lift_fixed_integer_relation`], retaining the exact source
/// carrier and opaque scalar operation of every embedded endpoint.
pub fn lower_integer_math_relation(proposition: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match proposition {
        Proposition::IntegerMathEqual(left, right) => (0, left, right),
        Proposition::IntegerMathLessThan(left, right) => (1, left, right),
        Proposition::IntegerMathLessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    crate::check_predicate_evaluation_size([proposition]).ok()?;
    let source_type = [left, right].into_iter().find_map(|term| match term {
        IntegerMathTerm::MathValue { source_type, .. } => Some(*source_type),
        IntegerMathTerm::Embed(term) => match term.scalar_type() {
            semantic_vocabulary::ScalarType::Integer(integer) if exact_ring_carrier(&integer) => {
                Some(integer)
            }
            _ => None,
        },
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
        IntegerMathTerm::Embed(term)
            if exact_ring_carrier(&source_type)
                && term.scalar_type() == semantic_vocabulary::ScalarType::Integer(source_type) =>
        {
            Some(term.as_ref().clone())
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
    exact_ring_carrier(&term.scalar_type().as_integer()?)
        .then(|| IntegerMathTerm::embed(term.clone()))
        .flatten()
}

/// Only `Fixed` non-address integer types denote exact ring operations as
/// the shared mathematical `add`/`subtract`/`multiply` functions.
fn exact_ring_carrier(scalar_type: &IntegerType) -> bool {
    scalar_type.carrier() == IntegerCarrier::Fixed && !scalar_type.is_address()
}

/// The kernel-facing relation lift behind
/// `PrimitiveJudgment::ClosedIntegerRelation`: unlike the bounded premise
/// matcher's [`lift_fixed_integer_relation`], the endpoints of an
/// `Equal`/`LessThan`/`LessOrEqual` lift *structurally* — `Exact`
/// add/subtract/multiply compounds on `Fixed` carriers become the same
/// `add`/`subtract`/`multiply` applications `fixed_scalar_term` already
/// denotes, so closed evaluation and open-term normalization can decide the
/// relation. The result keeps operand order; sorting and matching are the
/// canonical bridge's concern, not this derivation's.
pub(crate) fn lift_exact_integer_relation(
    proposition: &Proposition,
) -> Option<(IntegerMathTerm, IntegerMathTerm)> {
    crate::check_predicate_evaluation_size([proposition]).ok()?;
    let (left, right) = match proposition {
        semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
        _ => return None,
    };
    Some((
        lift_exact_integer_term(left)?,
        lift_exact_integer_term(right)?,
    ))
}

/// `Exact` ring operations on a `Fixed` non-address carrier denote the shared
/// `add`/`subtract`/`multiply` integer functions — the same denotation
/// `fixed_scalar_term` gives them — so they lift into the math vocabulary and
/// let the kernel's open-term normalization decide their identities. Every
/// other constructor keeps its existing carrier or opaque denotation: nothing
/// wrapping, saturating, trapping, divided or shifted is reinterpreted here.
pub(crate) fn lift_exact_integer_term(term: &ScalarTerm) -> Option<IntegerMathTerm> {
    match term {
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } if exact_ring_carrier(scalar_type) => Some(IntegerMathTerm::Add(
            Box::new(lift_exact_integer_term(left)?),
            Box::new(lift_exact_integer_term(right)?),
        )),
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } if exact_ring_carrier(scalar_type) => Some(IntegerMathTerm::Subtract(
            Box::new(lift_exact_integer_term(left)?),
            Box::new(lift_exact_integer_term(right)?),
        )),
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } if exact_ring_carrier(scalar_type) => Some(IntegerMathTerm::Multiply(
            Box::new(lift_exact_integer_term(left)?),
            Box::new(lift_exact_integer_term(right)?),
        )),
        // Ring expansion has a narrower purpose than opaque embedding. Its
        // callers request arithmetic rearrangement; an opaque projection or
        // policy-bearing operation cannot acquire those laws through fallback.
        ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } => lift_fixed_integer_term(term),
        _ => None,
    }
}

/// Preserve an exact finite order's denotation in the mathematical vocabulary.
/// This changes no arithmetic policy: only already-exact ring constructors
/// lift, and their resulting terms remain mathematical rather than new SSA
/// values or an assertion that an operation's finite result is representable.
pub fn lift_exact_integer_order(proposition: &Proposition) -> Option<Proposition> {
    let (left, right) = lift_exact_integer_relation(proposition)?;
    proposition.validate().ok()?;
    match proposition {
        Proposition::LessThan(..) => Some(Proposition::IntegerMathLessThan(left, right)),
        Proposition::LessOrEqual(..) => Some(Proposition::IntegerMathLessOrEqual(left, right)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IntegerSign, IntegerValue, PlaceId, PropositionContext, ScalarType, StructuralFieldId,
        StructuralPlaceKind, ValueId,
    };
    use terminal_psi::{ProofNode, ProofRule};

    #[test]
    fn embedded_endpoints_preserve_projection_and_operation_meaning() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let root = PlaceId::new(1).unwrap();
        let value = ValueId::new(1).unwrap();
        let context = PropositionContext::from_value_types_and_places(
            [(value, ScalarType::Integer(integer))],
            [(root, StructuralPlaceKind::Parameter { position: 0 })],
        )
        .unwrap();
        let field = ScalarTerm::integer_field_path(
            root,
            vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(1).unwrap(),
            )],
            integer,
        );
        let literal = ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap();
        let scalar = ScalarTerm::value(value, ScalarType::Integer(integer));
        let wrapping = ScalarTerm::WrappingIntegerAdd {
            scalar_type: integer,
            left: Box::new(scalar.clone()),
            right: Box::new(literal.clone()),
        };
        for endpoint in [field, wrapping] {
            let finite = Proposition::LessThan(endpoint.clone(), literal.clone());
            let mathematical = lift_fixed_integer_relation(&finite).unwrap();
            assert_eq!(
                lower_integer_math_relation(&mathematical),
                Some(finite.clone())
            );
            for (requirement, goal) in [
                (finite.clone(), mathematical.clone()),
                (mathematical.clone(), finite),
            ] {
                let proof = ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                };
                crate::check_certificate(
                    &context,
                    &goal,
                    std::slice::from_ref(&requirement),
                    &[],
                    &proof,
                )
                .unwrap();
                crate::verify_bounded_certificate(
                    &context,
                    &goal,
                    &[requirement],
                    &[],
                    &proof,
                    &mut crate::Budget::default(),
                )
                .unwrap();
            }
            let wrong = Proposition::IntegerMathLessThan(
                IntegerMathTerm::embed(scalar.clone()).unwrap(),
                IntegerMathTerm::literal(IntegerValue::Unsigned(10)),
            );
            let proof = ProofNode {
                conclusion: wrong.clone(),
                rule: ProofRule::Assumption { index: 0 },
            };
            assert!(
                crate::check_certificate(&context, &wrong, &[mathematical], &[], &proof).is_err()
            );
        }
        let other = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        let mixed = Proposition::IntegerMathLessThan(
            IntegerMathTerm::embed(scalar).unwrap(),
            IntegerMathTerm::MathValue {
                source_type: other,
                value,
            },
        );
        assert!(lower_integer_math_relation(&mixed).is_none());
        let outside = Proposition::IntegerMathLessThan(
            IntegerMathTerm::MathValue {
                source_type: integer,
                value,
            },
            IntegerMathTerm::literal(IntegerValue::Unsigned(256)),
        );
        assert!(lower_integer_math_relation(&outside).is_none());

        let mut deep = literal.clone();
        for _ in 0..65 {
            deep = ScalarTerm::WrappingIntegerAdd {
                scalar_type: integer,
                left: Box::new(deep),
                right: Box::new(literal.clone()),
            };
        }
        let oversized = Proposition::LessThan(deep, literal);
        assert!(lift_fixed_integer_relation(&oversized).is_none());
        assert!(lift_exact_integer_relation(&oversized).is_none());
        assert!(
            crate::decide_primitive(
                &context,
                &oversized,
                terminal_psi::PrimitiveJudgment::ClosedIntegerRelation
            )
            .is_err()
        );
    }
}
