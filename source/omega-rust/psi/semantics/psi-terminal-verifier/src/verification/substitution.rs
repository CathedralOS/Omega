//! Capture-free substitution for retained proposition, scalar, and content terms.

use std::collections::BTreeMap;

use psi_core::{
    CanonicalStructuralPathSegment, ContentConservation, ContentStructuralPlace, ContentTerm,
    IntegerMathTerm, PlaceId, Proposition, ScalarTerm, ValueId,
};

pub(crate) fn substitute_proposition_values(
    proposition: &Proposition,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Proposition {
    match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_values(left, substitutions),
            substitute_scalar_term_values(right, substitutions),
        ),
        Proposition::IntegerMathEqual(left, right) => Proposition::IntegerMathEqual(
            substitute_integer_math_term_values(left, substitutions),
            substitute_integer_math_term_values(right, substitutions),
        ),
        Proposition::IntegerMathLessThan(left, right) => Proposition::IntegerMathLessThan(
            substitute_integer_math_term_values(left, substitutions),
            substitute_integer_math_term_values(right, substitutions),
        ),
        Proposition::IntegerMathLessOrEqual(left, right) => Proposition::IntegerMathLessOrEqual(
            substitute_integer_math_term_values(left, substitutions),
            substitute_integer_math_term_values(right, substitutions),
        ),
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => Proposition::IeeeFloatComparison {
            kind: *kind,
            format: *format,
            left: left.clone(),
            right: right.clone(),
        },
        Proposition::ByteSequenceEqual { left, right } => Proposition::ByteSequenceEqual {
            left: left.clone(),
            right: right.clone(),
        },
        Proposition::StructuralCaseMembership { subject, case } => {
            Proposition::StructuralCaseMembership {
                subject: subject.clone(),
                case: *case,
            }
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| substitute_proposition_values(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_values(disjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_values(premise, substitutions)),
            conclusion: Box::new(substitute_proposition_values(conclusion, substitutions)),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(conservation.clone())
        }
    }
}

pub(crate) fn substitute_proposition_places(
    proposition: &Proposition,
    substitutions: &BTreeMap<PlaceId, PlaceId>,
) -> Proposition {
    let substitutions = substitutions
        .iter()
        .map(|(source, target)| (*source, (*target, Vec::new())))
        .collect::<BTreeMap<_, _>>();
    substitute_proposition_structural_places(proposition, &substitutions)
}

pub(crate) fn substitute_proposition_structural_places(
    proposition: &Proposition,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Proposition {
    match proposition {
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_places(left, substitutions),
            substitute_scalar_term_places(right, substitutions),
        ),
        Proposition::IntegerMathEqual(left, right) => {
            Proposition::IntegerMathEqual(left.clone(), right.clone())
        }
        Proposition::IntegerMathLessThan(left, right) => {
            Proposition::IntegerMathLessThan(left.clone(), right.clone())
        }
        Proposition::IntegerMathLessOrEqual(left, right) => {
            Proposition::IntegerMathLessOrEqual(left.clone(), right.clone())
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let rebase = |field: &psi_core::IeeeFloatStructuralField| {
                substitutions
                    .get(&field.root())
                    .map(|(root, prefix)| field.rebase(*root, prefix))
                    .unwrap_or_else(|| field.clone())
            };
            Proposition::IeeeFloatComparison {
                kind: *kind,
                format: *format,
                left: rebase(left),
                right: rebase(right),
            }
        }
        Proposition::ByteSequenceEqual { left, right } => {
            let rebase = |field: &psi_core::ByteSequenceStructuralField| {
                substitutions
                    .get(&field.root())
                    .map(|(root, prefix)| field.rebase(*root, prefix))
                    .unwrap_or_else(|| field.clone())
            };
            Proposition::ByteSequenceEqual {
                left: rebase(left),
                right: rebase(right),
            }
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            Proposition::StructuralCaseMembership {
                subject: substitutions
                    .get(&subject.root())
                    .map(|(root, prefix)| subject.rebase(*root, prefix))
                    .unwrap_or_else(|| subject.clone()),
                case: *case,
            }
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| substitute_proposition_structural_places(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_structural_places(disjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_structural_places(
                premise,
                substitutions,
            )),
            conclusion: Box::new(substitute_proposition_structural_places(
                conclusion,
                substitutions,
            )),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(ContentConservation::new(
                conservation.algebra().clone(),
                substitute_content_term_places(conservation.left(), substitutions),
                substitute_content_term_places(conservation.right(), substitutions),
            ))
        }
    }
}

fn substitute_integer_math_term_values(
    term: &IntegerMathTerm,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> IntegerMathTerm {
    match term {
        IntegerMathTerm::MathValue { source_type, value } => substitutions
            .get(value)
            .and_then(|replacement| match replacement {
                ScalarTerm::Value {
                    id,
                    scalar_type: psi_core::ScalarType::Integer(actual),
                } if actual == source_type => Some(IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                }),
                ScalarTerm::Integer {
                    scalar_type: actual,
                    value,
                } if actual == source_type => Some(IntegerMathTerm::literal(*value)),
                _ => None,
            })
            .unwrap_or_else(|| term.clone()),
        IntegerMathTerm::IntegerLiteral(_) => term.clone(),
        IntegerMathTerm::Add(left, right) => IntegerMathTerm::Add(
            Box::new(substitute_integer_math_term_values(left, substitutions)),
            Box::new(substitute_integer_math_term_values(right, substitutions)),
        ),
        IntegerMathTerm::Subtract(left, right) => IntegerMathTerm::Subtract(
            Box::new(substitute_integer_math_term_values(left, substitutions)),
            Box::new(substitute_integer_math_term_values(right, substitutions)),
        ),
        IntegerMathTerm::Multiply(left, right) => IntegerMathTerm::Multiply(
            Box::new(substitute_integer_math_term_values(left, substitutions)),
            Box::new(substitute_integer_math_term_values(right, substitutions)),
        ),
        IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
            value: Box::new(substitute_integer_math_term_values(value, substitutions)),
            count: Box::new(substitute_integer_math_term_values(count, substitutions)),
        },
    }
}

fn substitute_scalar_term_places(
    term: &ScalarTerm,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ScalarTerm {
    let mut term = term.clone();
    fn substitute(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                if let Some((replacement, prefix)) = substitutions.get(root) {
                    *root = *replacement;
                    if !prefix.is_empty() {
                        let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                        rebased.extend(prefix);
                        rebased.append(path);
                        *path = rebased;
                    }
                }
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => substitute(operand, substitutions),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                substitute(left, substitutions);
                substitute(right, substitutions);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute(value, substitutions);
                substitute(count, substitutions);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }
    substitute(&mut term, substitutions);
    term
}

fn substitute_content_term_places(
    term: &ContentTerm,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ContentTerm {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => ContentTerm::Projection {
            projection: *projection,
            subject: ContentStructuralPlace {
                version: subject.version,
                root: substitutions
                    .get(&subject.root)
                    .map(|(root, _)| *root)
                    .unwrap_or(subject.root),
                segments: subject.segments.clone(),
            },
        },
        ContentTerm::Separate(terms) => ContentTerm::Separate(
            terms
                .iter()
                .map(|term| substitute_content_term_places(term, substitutions))
                .collect(),
        ),
    }
}

pub(super) fn substitute_scalar_term_values(
    term: &ScalarTerm,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> ScalarTerm {
    let recurse = |term: &ScalarTerm| substitute_scalar_term_values(term, substitutions);
    match term {
        ScalarTerm::Value { id, .. } => substitutions
            .get(id)
            .cloned()
            .unwrap_or_else(|| term.clone()),
        ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. } => term.clone(),
        ScalarTerm::BooleanNot { operand } => ScalarTerm::BooleanNot {
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => ScalarTerm::IntegerBitwiseNot {
            scalar_type: *scalar_type,
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => ScalarTerm::IntegerWiden {
            source_type: *source_type,
            target_type: *target_type,
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => ScalarTerm::IntegerExactCast {
            source_type: *source_type,
            target_type: *target_type,
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::BooleanEqual { left, right } => ScalarTerm::BooleanEqual {
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerEqual {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerLessThan {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerLessOrEqual {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerBitwiseAnd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerBitwiseOr {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => ScalarTerm::IntegerBitwiseXor {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::WrappingIntegerShiftLeft {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::WrappingIntegerShiftRight {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::ExactIntegerShiftRight {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => ScalarTerm::ExactIntegerShiftLeft {
            value_type: *value_type,
            count_type: *count_type,
            value: Box::new(recurse(value)),
            count: Box::new(recurse(count)),
        },
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerAdd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerSubtract {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerMultiply {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerDivide {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => ScalarTerm::ExactIntegerRemainder {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerDivide {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerRemainder {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerDivide {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerRemainder {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerAdd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerAdd {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerSubtract {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerSubtract {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => ScalarTerm::WrappingIntegerMultiply {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => ScalarTerm::SaturatingIntegerMultiply {
            scalar_type: *scalar_type,
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
    }
}
