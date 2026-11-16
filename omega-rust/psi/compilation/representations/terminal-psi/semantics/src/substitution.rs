/*
Capture-avoiding term, proposition, and contract substitution across scalar values, structural
places, and callable boundaries.
*/

use std::collections::BTreeMap;

#[path = "substitution_clauses.rs"]
pub mod clauses;

use terminal_psi::{CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

use semantic_vocabulary::ScalarReadProjection;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentConservation, ContentStructuralPlace, ContentTerm,
    IntegerMathTerm, PlaceId, Proposition, ScalarTerm, ValueId,
};

pub fn substitute_proposition_values(
    proposition: &Proposition,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Proposition {
    substitute_proposition_values_with(proposition, &|identity| substitutions.get(&identity))
}

pub fn substitute_proposition_values_with<'input>(
    proposition: &Proposition,
    substitutions: &dyn Fn(ValueId) -> Option<&'input ScalarTerm>,
) -> Proposition {
    match proposition {
        Proposition::FloatMeaningEqual(clause) => {
            Proposition::FloatMeaningEqual(clause.map_sources_or_invalid(|source| {
                substitute_scalar_term_values_with(source, substitutions)
            }))
        }
        Proposition::FloatIntegerConversion(clause) => {
            let mut mapped = clause.clone();
            mapped.source = substitute_scalar_term_values_with(&clause.source, substitutions);
            mapped.result = substitute_scalar_term_values_with(&clause.result, substitutions);
            Proposition::FloatIntegerConversion(mapped)
        }
        Proposition::IeeeFloatIntegerExact { source, target } => {
            Proposition::IeeeFloatIntegerExact {
                source: substitute_scalar_term_values_with(source, substitutions),
                target: *target,
            }
        }
        Proposition::Truth => Proposition::Truth,
        Proposition::Falsehood => Proposition::Falsehood,
        Proposition::Atom(atom) => Proposition::Atom(*atom),
        Proposition::Equal(left, right) => Proposition::Equal(
            substitute_scalar_term_values_with(left, substitutions),
            substitute_scalar_term_values_with(right, substitutions),
        ),
        Proposition::LessThan(left, right) => Proposition::LessThan(
            substitute_scalar_term_values_with(left, substitutions),
            substitute_scalar_term_values_with(right, substitutions),
        ),
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(
            substitute_scalar_term_values_with(left, substitutions),
            substitute_scalar_term_values_with(right, substitutions),
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
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let mut left = substitute_scalar_term_values_with(left, substitutions);
            let mut right = substitute_scalar_term_values_with(right, substitutions);
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::ScalarIeeeFloatComparison {
                kind: *kind,
                format: *format,
                left,
                right,
            }
        }
        Proposition::ByteSequenceEqual { left, right } => Proposition::ByteSequenceEqual {
            left: left.clone(),
            right: right.clone(),
        },
        Proposition::StructuralIdentity { left, right } => Proposition::StructuralIdentity {
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
                .map(|conjunct| substitute_proposition_values_with(conjunct, substitutions))
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| substitute_proposition_values_with(disjunct, substitutions))
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_values_with(premise, substitutions)),
            conclusion: Box::new(substitute_proposition_values_with(
                conclusion,
                substitutions,
            )),
        },
        Proposition::ContentConservation(conservation) => {
            Proposition::ContentConservation(conservation.clone())
        }
    }
}

pub fn substitute_proposition_structural_places(
    proposition: &Proposition,
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Proposition {
    substitute_proposition_structural_places_with(proposition, &|identity| {
        substitutions.get(&identity)
    })
}

pub fn substitute_proposition_structural_places_with<'input>(
    proposition: &Proposition,
    substitutions: &dyn Fn(
        PlaceId,
    ) -> Option<&'input (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Proposition {
    match proposition {
        Proposition::FloatMeaningEqual(clause) => {
            Proposition::FloatMeaningEqual(clause.map_sources_or_invalid(|source| {
                substitute_scalar_term_places(source, substitutions)
            }))
        }
        Proposition::FloatIntegerConversion(clause) => {
            let mut mapped = clause.clone();
            mapped.source = substitute_scalar_term_places(&clause.source, substitutions);
            mapped.result = substitute_scalar_term_places(&clause.result, substitutions);
            Proposition::FloatIntegerConversion(mapped)
        }
        Proposition::IeeeFloatIntegerExact { source, target } => {
            Proposition::IeeeFloatIntegerExact {
                source: substitute_scalar_term_places(source, substitutions),
                target: *target,
            }
        }
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
        Proposition::IntegerMathEqual(left, right) => Proposition::IntegerMathEqual(
            substitute_integer_math_term_places(left, substitutions),
            substitute_integer_math_term_places(right, substitutions),
        ),
        Proposition::IntegerMathLessThan(left, right) => Proposition::IntegerMathLessThan(
            substitute_integer_math_term_places(left, substitutions),
            substitute_integer_math_term_places(right, substitutions),
        ),
        Proposition::IntegerMathLessOrEqual(left, right) => Proposition::IntegerMathLessOrEqual(
            substitute_integer_math_term_places(left, substitutions),
            substitute_integer_math_term_places(right, substitutions),
        ),
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let rebase = |field: &semantic_vocabulary::IeeeFloatStructuralField| {
                substitutions(field.root())
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
        Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            let mut left = substitute_scalar_term_places(left, substitutions);
            let mut right = substitute_scalar_term_places(right, substitutions);
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::ScalarIeeeFloatComparison {
                kind: *kind,
                format: *format,
                left,
                right,
            }
        }
        Proposition::ByteSequenceEqual { left, right } => {
            let rebase = |field: &semantic_vocabulary::ByteSequenceStructuralField| {
                substitutions(field.root())
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
                subject: substitutions(subject.root())
                    .map(|(root, prefix)| subject.rebase(*root, prefix))
                    .unwrap_or_else(|| subject.clone()),
                case: *case,
            }
        }
        Proposition::StructuralIdentity { left, right } => {
            let rebase = |subject: &semantic_vocabulary::StructuralIdentitySubject| {
                substitutions(subject.root())
                    .map(|(root, prefix)| subject.rebase(*root, prefix))
                    .unwrap_or_else(|| subject.clone())
            };
            let mut left = rebase(left);
            let mut right = rebase(right);
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Proposition::StructuralIdentity { left, right }
        }
        Proposition::Conjunction(conjuncts) => Proposition::Conjunction(
            conjuncts
                .iter()
                .map(|conjunct| {
                    substitute_proposition_structural_places_with(conjunct, substitutions)
                })
                .collect(),
        ),
        Proposition::Disjunction(disjuncts) => Proposition::Disjunction(
            disjuncts
                .iter()
                .map(|disjunct| {
                    substitute_proposition_structural_places_with(disjunct, substitutions)
                })
                .collect(),
        ),
        Proposition::Implication {
            premise,
            conclusion,
        } => Proposition::Implication {
            premise: Box::new(substitute_proposition_structural_places_with(
                premise,
                substitutions,
            )),
            conclusion: Box::new(substitute_proposition_structural_places_with(
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

fn substitute_integer_math_term_values<'input>(
    term: &IntegerMathTerm,
    substitutions: &dyn Fn(ValueId) -> Option<&'input ScalarTerm>,
) -> IntegerMathTerm {
    match term {
        IntegerMathTerm::Embed(scalar) => {
            IntegerMathTerm::embed(substitute_scalar_term_values_with(scalar, substitutions))
                .unwrap_or_else(|| term.clone())
        }
        IntegerMathTerm::MathValue { source_type, value } => substitutions(*value)
            .and_then(|replacement| {
                (replacement.scalar_type().as_integer() == Some(*source_type))
                    .then(|| IntegerMathTerm::embed(replacement.clone()))
                    .flatten()
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

fn substitute_integer_math_term_places<'input>(
    term: &IntegerMathTerm,
    substitutions: &dyn Fn(
        PlaceId,
    ) -> Option<&'input (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> IntegerMathTerm {
    let mut term = term.clone();
    let mut pending = vec![&mut term];
    while let Some(term) = pending.pop() {
        if let IntegerMathTerm::Embed(scalar) = term {
            **scalar = substitute_scalar_term_places(scalar, substitutions);
        } else {
            pending.extend(term.operands_mut());
        }
    }
    term
}

fn substitute_scalar_term_places<'input>(
    term: &ScalarTerm,
    substitutions: &dyn Fn(
        PlaceId,
    ) -> Option<&'input (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ScalarTerm {
    let mut term = term.clone();
    fn substitute<'input>(
        term: &mut ScalarTerm,
        substitutions: &dyn Fn(
            PlaceId,
        )
            -> Option<&'input (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) {
        match term {
            ScalarTerm::StructuralRead { root, path, .. } => {
                if let Some((replacement, prefix)) = substitutions(*root) {
                    *root = *replacement;
                    let mut rebased = prefix
                        .iter()
                        .copied()
                        .map(ScalarReadProjection::from)
                        .collect::<Vec<_>>();
                    rebased.append(path);
                    *path = rebased;
                }
                for projection in path {
                    if let ScalarReadProjection::Index(index) = projection {
                        substitute(index, substitutions);
                    }
                }
            }
            semantic_vocabulary::scalar_term_predicate_quint_pattern!(root, path, ..) => {
                if let Some((replacement, prefix)) = substitutions(*root) {
                    *root = *replacement;
                    if !prefix.is_empty() {
                        let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                        rebased.extend(prefix);
                        rebased.append(path);
                        *path = rebased;
                    }
                }
            }
            semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                substitute(operand, substitutions)
            }
            semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                substitute(left, substitutions);
                substitute(right, substitutions);
            }
            ScalarTerm::Value { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_)
            | ScalarTerm::Application { .. } => {}
        }
    }
    substitute(&mut term, substitutions);
    term
}

fn substitute_content_term_places<'input>(
    term: &ContentTerm,
    substitutions: &dyn Fn(
        PlaceId,
    ) -> Option<&'input (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> ContentTerm {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => ContentTerm::Projection {
            projection: *projection,
            subject: ContentStructuralPlace {
                version: subject.version,
                root: substitutions(subject.root)
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

pub(crate) const SCALAR_EXPRESSION_DEPTH: usize = 64;
pub(crate) const SCALAR_EXPRESSION_NODES: usize = 4096;

pub(crate) fn substitute_bound_scalar_term(
    term: &ScalarTerm,
    arguments: &[(ValueId, ScalarTerm)],
) -> Option<ScalarTerm> {
    let argument = |id| {
        let position = arguments
            .binary_search_by_key(&id, |(value, _)| *value)
            .ok()?;
        Some(&arguments[position].1)
    };
    // Bound composition before cloning an argument at every occurrence.
    // Arguments belong to the caller, so their own ValueIds are not bound
    // again, even when they have the same number as a callee ValueId.
    let mut remaining = SCALAR_EXPRESSION_NODES;
    let mut pending = vec![(term, true, 0)];
    while let Some((mut term, mut bind, depth)) = pending.pop() {
        if depth > SCALAR_EXPRESSION_DEPTH {
            return None;
        }
        remaining = remaining.checked_sub(1)?;
        if let ScalarTerm::Value { id, scalar_type } = term
            && bind
        {
            term = argument(*id)?;
            if term.scalar_type() != *scalar_type {
                return None;
            }
            bind = false;
        }
        pending.extend(
            term.operands()
                .into_iter()
                .map(|term| (term, bind, depth + 1)),
        );
    }
    Some(substitute_scalar_term_values_with(term, &argument))
}

pub(crate) fn substitute_scalar_term_values(
    term: &ScalarTerm,
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> ScalarTerm {
    substitute_scalar_term_values_with(term, &|identity| substitutions.get(&identity))
}

pub(crate) fn substitute_scalar_term_values_with<'input>(
    term: &ScalarTerm,
    substitutions: &dyn Fn(ValueId) -> Option<&'input ScalarTerm>,
) -> ScalarTerm {
    let recurse = |term: &ScalarTerm| substitute_scalar_term_values_with(term, substitutions);
    macro_rules! typed_binary_body {
        ($variant:ident, $scalar_type:ident, $left:ident, $right:ident) => {
            ScalarTerm::$variant {
                scalar_type: *$scalar_type,
                left: Box::new(recurse($left)),
                right: Box::new(recurse($right)),
            }
        };
    }
    macro_rules! typed_unary_body {
        ($variant:ident, $scalar_type:ident, $operand:ident) => {
            ScalarTerm::$variant {
                scalar_type: *$scalar_type,
                operand: Box::new(recurse($operand)),
            }
        };
    }
    macro_rules! shift_body {
        ($variant:ident, $value_type:ident, $count_type:ident, $value:ident, $count:ident) => {
            ScalarTerm::$variant {
                value_type: *$value_type,
                count_type: *$count_type,
                value: Box::new(recurse($value)),
                count: Box::new(recurse($count)),
            }
        };
    }
    macro_rules! cast_body {
        ($variant:ident, $source_type:ident, $target_type:ident, $operand:ident) => {
            ScalarTerm::$variant {
                source_type: *$source_type,
                target_type: *$target_type,
                operand: Box::new(recurse($operand)),
            }
        };
    }
    match term {
        ScalarTerm::Value { id, .. } => substitutions(*id).cloned().unwrap_or_else(|| term.clone()),
        ScalarTerm::StructuralRead {
            root,
            path,
            scalar_type,
        } => ScalarTerm::StructuralRead {
            root: *root,
            path: path
                .iter()
                .map(|projection| match projection {
                    ScalarReadProjection::Referent => ScalarReadProjection::Referent,
                    ScalarReadProjection::Extent => ScalarReadProjection::Extent,
                    ScalarReadProjection::Static(segment) => ScalarReadProjection::Static(*segment),
                    ScalarReadProjection::Index(index) => {
                        ScalarReadProjection::Index(Box::new(recurse(index)))
                    }
                })
                .collect(),
            scalar_type: *scalar_type,
        },
        semantic_vocabulary::scalar_term_predicate_quint_pattern!(..)
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. }
        | ScalarTerm::IeeeFloatLiteral(_)
        | ScalarTerm::Application { .. } => term.clone(),
        ScalarTerm::BooleanNot { operand } => ScalarTerm::BooleanNot {
            operand: Box::new(recurse(operand)),
        },
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => typed_unary_body!(IntegerBitwiseNot, scalar_type, operand),
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => cast_body!(IntegerWiden, source_type, target_type, operand),
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => cast_body!(IntegerExactCast, source_type, target_type, operand),
        ScalarTerm::BooleanEqual { left, right } => ScalarTerm::BooleanEqual {
            left: Box::new(recurse(left)),
            right: Box::new(recurse(right)),
        },
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerEqual, scalar_type, left, right),
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerLessThan, scalar_type, left, right),
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerLessOrEqual, scalar_type, left, right),
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerBitwiseAnd, scalar_type, left, right),
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerBitwiseOr, scalar_type, left, right),
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(IntegerBitwiseXor, scalar_type, left, right),
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(
            WrappingIntegerShiftLeft,
            value_type,
            count_type,
            value,
            count
        ),
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(
            WrappingIntegerShiftRight,
            value_type,
            count_type,
            value,
            count
        ),
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(ExactIntegerShiftRight, value_type, count_type, value, count),
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => shift_body!(ExactIntegerShiftLeft, value_type, count_type, value, count),
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerAdd, scalar_type, left, right),
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerSubtract, scalar_type, left, right),
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerMultiply, scalar_type, left, right),
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerDivide, scalar_type, left, right),
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(ExactIntegerRemainder, scalar_type, left, right),
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerDivide, scalar_type, left, right),
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerRemainder, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerDivide, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerRemainder, scalar_type, left, right),
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerAdd, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerAdd, scalar_type, left, right),
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerSubtract, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerSubtract, scalar_type, left, right),
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(WrappingIntegerMultiply, scalar_type, left, right),
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => typed_binary_body!(SaturatingIntegerMultiply, scalar_type, left, right),
    }
}

pub fn substitute_crash_routes(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<ValueId, ScalarTerm>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .filter_map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => Some(CrashRouteGuard::Truth),
                    CrashRouteGuard::Predicate(predicate) => {
                        match substitute_proposition_values(predicate.proposition(), substitutions)
                        {
                            Proposition::Truth => Some(CrashRouteGuard::Truth),
                            Proposition::Falsehood => None,
                            proposition => Some(CrashRouteGuard::Predicate(
                                CrashPredicateTerm::new(proposition),
                            )),
                        }
                    }
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            (!alternatives.is_empty()).then_some(CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            })
        })
        .collect()
}
