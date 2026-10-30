//! Case atoms consume exact tag evidence without reading inactive payloads.
//! Folding happens in the source predicate's authenticated scope, before its
//! remaining observations are transported to another value namespace.
use super::{Boolean, BoundObservation, BoundPredicate, Operand};

pub(crate) fn fold_cases(
    predicate: &mut BoundPredicate,
    mut resolve: impl FnMut(&BoundObservation, &typed_trees::data::DataMemberIdentity) -> Option<bool>,
) {
    fn visit(
        expression: &mut Boolean,
        bindings: &[BoundObservation],
        resolve: &mut impl FnMut(
            &BoundObservation,
            &typed_trees::data::DataMemberIdentity,
        ) -> Option<bool>,
    ) {
        match expression {
            Boolean::StructuralCaseMembership { subject, case } => {
                let operand = Operand::CaseSubject(subject.clone());
                if let Some(binding) = bindings.iter().find(|binding| binding.operand == operand)
                    && let Some(value) = resolve(binding, case)
                {
                    *expression = Boolean::Constant(value);
                }
            }
            Boolean::Not(operand) => visit(operand, bindings, resolve),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                visit(left, bindings, resolve);
                visit(right, bindings, resolve);
            }
            _ => {}
        }
    }
    visit(&mut predicate.expression, &predicate.bindings, &mut resolve);
    super::simplify(&mut predicate.expression);
}
