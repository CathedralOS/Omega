//! Exact extent observation of an established immutable subslice.

use semantic_vocabulary::{IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType};

use super::StructuralEffectObservation;
use crate::OperationSemanticError;

/// Returns the subslice's direct extent denotation at a matching length read.
/// The caller must establish the exact defining producer and its dominance;
/// the producer's two bounds remain independent required proof obligations.
/// No equation is returned for other producers or a different measured view.
pub fn subslice_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let StructuralEffectObservation::ByteSequenceLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let StructuralEffectObservation::ByteSequenceSubslice {
        destination,
        start,
        end,
        ..
    } = producer
    else {
        return Ok(None);
    };
    if source != destination {
        return Ok(None);
    }
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
    let difference = ScalarTerm::exact_integer_subtract(integer_type, value(*end), value(*start))
        .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(value(*result), difference)))
}

#[cfg(test)]
mod tests {
    use super::subslice_length_equation;
    use crate::structural_effect::StructuralEffectObservation;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, ObligationId, PlaceId, Proposition, ScalarTerm, ScalarType,
        ValueId,
    };

    fn value(identity: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        )
    }

    fn subslice(start: u64, end: u64) -> StructuralEffectObservation {
        StructuralEffectObservation::ByteSequenceSubslice {
            source: PlaceId::new(1).unwrap(),
            start: ValueId::new(start).unwrap(),
            end: ValueId::new(end).unwrap(),
            length: ValueId::new(5).unwrap(),
            destination: PlaceId::new(2).unwrap(),
            obligation: ObligationId::new(6).unwrap(),
        }
    }

    fn length_read(source: u64, result: u64) -> StructuralEffectObservation {
        StructuralEffectObservation::ByteSequenceLengthRead {
            source: PlaceId::new(source).unwrap(),
            result: ValueId::new(result).unwrap(),
        }
    }

    fn assert_equation(equation: &Proposition, result: u64, end: u64, start: u64) {
        let Proposition::Equal(
            measured,
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            },
        ) = equation
        else {
            panic!("the measured length equals exact end minus start")
        };
        assert_eq!(*measured, value(result));
        assert_eq!(
            *scalar_type,
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
        );
        assert_eq!(**left, value(end));
        assert_eq!(**right, value(start));
        equation.validate().unwrap();
    }

    #[test]
    fn matching_length_retains_result_endpoints_and_copied_observations() {
        let producer = subslice(3, 4);
        let observation = length_read(2, 7);
        let equation = subslice_length_equation(&observation, &producer)
            .unwrap()
            .unwrap();
        assert_equation(&equation, 7, 4, 3);
        assert_eq!(
            subslice_length_equation(&observation.clone(), &producer.clone()).unwrap(),
            Some(equation),
        );
        let later = subslice_length_equation(&length_read(2, 8), &producer)
            .unwrap()
            .unwrap();
        assert_equation(&later, 8, 4, 3);
        assert_eq!(observation.local_equation(), None);
        assert_eq!(producer.local_equation(), None);
        assert_eq!(
            producer.canonical_obligation(),
            Some((
                ObligationId::new(6).unwrap(),
                Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(value(3), value(4)),
                    Proposition::LessOrEqual(value(4), value(5)),
                ]),
            )),
        );
    }

    #[test]
    fn empty_window_keeps_exact_difference_and_both_bounds() {
        let producer = subslice(5, 5);
        let equation = subslice_length_equation(&length_read(2, 7), &producer)
            .unwrap()
            .unwrap();
        assert_equation(&equation, 7, 5, 5);
        assert_eq!(
            producer.canonical_obligation().unwrap().1,
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(value(5), value(5)),
                Proposition::LessOrEqual(value(5), value(5)),
            ]),
        );
        assert_eq!(producer.local_equation(), None);
    }

    #[test]
    fn reversed_endpoints_are_not_normalized() {
        let observation = length_read(2, 7);
        let forward = subslice_length_equation(&observation, &subslice(3, 4))
            .unwrap()
            .unwrap();
        let reversed = subslice_length_equation(&observation, &subslice(4, 3))
            .unwrap()
            .unwrap();
        assert_equation(&reversed, 7, 3, 4);
        assert_ne!(forward, reversed);
        assert_eq!(
            subslice(4, 3).canonical_obligation().unwrap().1,
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(value(4), value(3)),
                Proposition::LessOrEqual(value(3), value(5)),
            ]),
        );
    }

    #[test]
    fn original_or_unrelated_source_does_not_supply_derived_extent() {
        for source in [1, 9] {
            assert_eq!(
                subslice_length_equation(&length_read(source, 7), &subslice(3, 4)),
                Ok(None),
            );
        }
    }

    #[test]
    fn wrong_observation_or_producer_kind_is_declined() {
        let observation = length_read(2, 7);
        let producer = subslice(3, 4);
        let literal = StructuralEffectObservation::ByteSequencePlaceEstablished {
            destination: PlaceId::new(2).unwrap(),
        };
        for (observation, producer) in [
            (&producer, &observation),
            (&observation, &observation),
            (&producer, &producer),
            (&observation, &literal),
            (&literal, &producer),
        ] {
            assert_eq!(subslice_length_equation(observation, producer), Ok(None));
        }
    }
}
