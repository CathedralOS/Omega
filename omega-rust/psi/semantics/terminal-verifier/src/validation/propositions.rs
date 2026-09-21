//! Projects exact structural roots from retained terminal propositions.

use super::{
    BTreeSet, CanonicalStructuralPathSegment, ContentTerm, PlaceId, Proposition, ScalarTerm,
};
/// Unversioned observations of a written root cannot describe its new contents.
/// Until a checked write frame preserves individual paths, forget the complete
/// root, including observations nested under logical or arithmetic operators.
pub(crate) fn proposition_observes_places(proposition: &Proposition, places: &[PlaceId]) -> bool {
    proposition_boolean_field_roots(proposition)
        .into_iter()
        .chain(proposition_content_roots(proposition))
        .any(|root| places.contains(&root))
}

/// One checked field write names an exact canonical path beneath its root.
/// Disjoint sibling paths cannot observe the replacement, so their facts stay
/// current; only observations that cover or enter the written path expire.
/// Observations without an exact canonical path (content projections keep
/// name-spelled segments) still forget the complete root.
pub(crate) fn proposition_observes_write(
    proposition: &Proposition,
    root: PlaceId,
    written: &[CanonicalStructuralPathSegment],
) -> bool {
    proposition_observation_sites(proposition)
        .into_iter()
        .any(|(site_root, site_path)| {
            site_root == root
                && match site_path {
                    None => true,
                    Some(path) => path.starts_with(written) || written.starts_with(&path),
                }
        })
}

/// `Some` projects the exact canonical leaf path; `None` is a whole-root
/// observation whose finer structure this judgment cannot yet name.
type ObservationSite = (PlaceId, Option<Vec<CanonicalStructuralPathSegment>>);

fn proposition_observation_sites(proposition: &Proposition) -> Vec<ObservationSite> {
    fn collect_term(term: &ScalarTerm, sites: &mut Vec<ObservationSite>) {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                sites.push((*root, Some(path.clone())));
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => collect_term(operand, sites),
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
                collect_term(left, sites);
                collect_term(right, sites);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                collect_term(value, sites);
                collect_term(count, sites);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    fn collect(proposition: &Proposition, sites: &mut Vec<ObservationSite>) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                collect_term(left, sites);
                collect_term(right, sites);
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                sites.push((left.root(), Some(left.path().to_vec())));
                sites.push((right.root(), Some(right.path().to_vec())));
            }
            Proposition::ByteSequenceEqual { left, right } => {
                sites.push((left.root(), Some(left.path().to_vec())));
                sites.push((right.root(), Some(right.path().to_vec())));
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                sites.push((subject.root(), Some(subject.path().to_vec())));
            }
            Proposition::ContentConservation(conservation) => {
                fn collect_content(term: &ContentTerm, sites: &mut Vec<ObservationSite>) {
                    match term {
                        ContentTerm::Projection { subject, .. } => {
                            sites.push((subject.root, None));
                        }
                        ContentTerm::Separate(terms) => {
                            for term in terms {
                                collect_content(term, sites);
                            }
                        }
                    }
                }
                collect_content(conservation.left(), sites);
                collect_content(conservation.right(), sites);
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, sites);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, sites);
                collect(conclusion, sites);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _) => {}
        }
    }

    let mut sites = Vec::new();
    collect(proposition, &mut sites);
    sites
}

/// Field and case observations have no entry/current revision. Content terms
/// retain their separate revision and conservation rules across ownership moves.
pub(crate) fn proposition_observes_unversioned_places(
    proposition: &Proposition,
    places: &[PlaceId],
) -> bool {
    proposition_boolean_field_roots(proposition)
        .into_iter()
        .any(|root| places.contains(&root))
}

pub(super) fn proposition_contains_content(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::ContentConservation(_) => true,
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            propositions.iter().any(proposition_contains_content)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_contains_content(premise) || proposition_contains_content(conclusion),
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::Equal(_, _)
        | Proposition::LessThan(_, _)
        | Proposition::LessOrEqual(_, _)
        | Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _)
        | Proposition::IeeeFloatComparison { .. }
        | Proposition::ScalarIeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralCaseMembership { .. } => false,
    }
}

pub(super) fn proposition_boolean_field_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    proposition_observation_sites(proposition)
        .into_iter()
        .filter(|(_, path)| path.is_some())
        .map(|(root, _)| root)
        .collect()
}

pub(super) fn proposition_content_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    proposition_observation_sites(proposition)
        .into_iter()
        .filter(|(_, path)| path.is_none())
        .map(|(root, _)| root)
        .collect()
}
