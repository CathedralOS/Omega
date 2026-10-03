//! A closed predicate over one scalar, independent of a machine's SSA namespace.
//!
//! Field validity is a predicate, not a second executable carrier or a nominal
//! tag. Retaining the ordinary proposition algebra avoids separate encodings for
//! intervals, nonzero values and each Boolean combination. The single formal is
//! checked in an isolated context: an ambient value, place or opaque atom cannot
//! smuggle a premise into a declaration. Instantiation only binds that formal;
//! it neither proves the predicate nor grants custody of a field.

use crate::{Proposition, PropositionContext, PropositionError, ScalarType, ValueId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarRefinement {
    carrier: ScalarType,
    predicate: Proposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarRefinementError {
    NotClosed,
    InvalidPredicate(PropositionError),
}

impl ScalarRefinement {
    /// This coordinate belongs to the declaration-local namespace, never to
    /// an enclosing machine. Consumers instantiate it before using the formula.
    pub const SUBJECT: ValueId = ValueId::FIRST;

    pub fn new(carrier: ScalarType, predicate: Proposition) -> Result<Self, ScalarRefinementError> {
        // Decoders and source producers share this admission. The node budget
        // also bounds later clones; visiting an incomplete prefix cannot admit.
        let mut remaining = 4096;
        let mut foreign_value = false;
        if predicate.visit_value_ids_bounded(&mut remaining, |value| {
            foreign_value |= value != Self::SUBJECT;
        }) != Some(true)
            || foreign_value
        {
            return Err(ScalarRefinementError::NotClosed);
        }
        let context = PropositionContext::from_value_types([(Self::SUBJECT, carrier)])
            .map_err(ScalarRefinementError::InvalidPredicate)?;
        context
            .validate(&predicate)
            .map_err(ScalarRefinementError::InvalidPredicate)?;
        Ok(Self { carrier, predicate })
    }

    pub fn carrier(&self) -> ScalarType {
        self.carrier
    }

    pub fn predicate(&self) -> &Proposition {
        &self.predicate
    }

    /// The exact declaration requirement on an initializer or observed snapshot.
    /// The caller must independently establish that value's carrier and origin.
    pub fn instantiate(&self, value: ValueId) -> Proposition {
        let mut predicate = self.predicate.clone();
        predicate.remap_value_ids(|_| value);
        predicate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarTerm};

    #[test]
    fn compound_mathematical_predicates_keep_their_operators_and_literals() {
        let integer = IntegerType::new(IntegerSign::Signed, 64).unwrap();
        let subject = IntegerMathTerm::MathValue {
            source_type: integer,
            value: ScalarRefinement::SUBJECT,
        };
        let predicate = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Multiply(Box::new(subject.clone()), Box::new(subject)),
            IntegerMathTerm::literal(IntegerValue::Signed(100)),
        );
        let refinement =
            ScalarRefinement::new(ScalarType::Integer(integer), predicate.clone()).unwrap();
        let value = ValueId::new(7).unwrap();
        let instantiated = refinement.instantiate(value);
        let mut expected = predicate;
        expected.remap_value_ids(|_| value);
        assert_eq!(instantiated, expected);
        let Proposition::IntegerMathLessOrEqual(IntegerMathTerm::Multiply(left, right), literal) =
            instantiated
        else {
            panic!("substitution preserves mathematical multiplication and comparison");
        };
        assert_eq!(
            *left,
            IntegerMathTerm::MathValue {
                source_type: integer,
                value,
            },
        );
        assert_eq!(left, right);
        assert_eq!(literal, IntegerMathTerm::literal(IntegerValue::Signed(100)));
    }

    #[test]
    fn nonzero_is_an_ordinary_disjunction_over_the_exact_snapshot() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let carrier = ScalarType::Integer(integer);
        let subject = ScalarTerm::value(ScalarRefinement::SUBJECT, carrier);
        let zero = ScalarTerm::integer(integer, crate::IntegerValue::Signed(0)).unwrap();
        let refinement = ScalarRefinement::new(
            carrier,
            Proposition::Disjunction(vec![
                Proposition::LessThan(subject.clone(), zero.clone()),
                Proposition::LessThan(zero, subject),
            ]),
        )
        .unwrap();
        let value = ValueId::new(17).unwrap();
        let instantiated = refinement.instantiate(value);
        let mut values = Vec::new();
        assert!(instantiated.visit_value_ids(|value| values.push(value)));
        assert_eq!(values, vec![value, value]);
        PropositionContext::from_value_types([(value, carrier)])
            .unwrap()
            .validate(&instantiated)
            .unwrap();
    }

    #[test]
    fn foreign_values_structural_observations_and_opaque_assumptions_reject() {
        for predicate in [
            Proposition::Equal(
                ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
                ScalarTerm::boolean(true),
            ),
            Proposition::Equal(
                ScalarTerm::boolean_field_path(PlaceId::new(1).unwrap(), Vec::new()),
                ScalarTerm::boolean(true),
            ),
            Proposition::Atom(crate::PropositionId::new(1).unwrap()),
        ] {
            assert_eq!(
                ScalarRefinement::new(ScalarType::Boolean, predicate),
                Err(ScalarRefinementError::NotClosed),
            );
        }
    }

    #[test]
    fn a_subject_cannot_be_reinterpreted_at_another_carrier() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let subject = ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Integer(integer));
        assert!(matches!(
            ScalarRefinement::new(
                ScalarType::Boolean,
                Proposition::Equal(subject.clone(), subject)
            ),
            Err(ScalarRefinementError::InvalidPredicate(_)),
        ));
    }
}
