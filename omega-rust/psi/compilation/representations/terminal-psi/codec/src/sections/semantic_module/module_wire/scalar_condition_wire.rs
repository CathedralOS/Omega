//! Source-free captured bounds. Wire order is condition identity order.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::wire::{Reader, Writer};
use numerics::bignum::BigInt;
use numerics::integer_bounds::IntegerBoundStep;
use terminal_psi::{
    ScalarConditionSubject, ScalarConditions, ScalarRangeCondition, ScalarRangeObservation,
    ScalarRangeRequirement,
};

fn encode_integer(writer: &mut Writer, value: &BigInt) -> Result<(), CodecError> {
    writer.boolean(value.is_negative());
    writer.len("bound magnitude words", value.magnitude_words().len())?;
    for word in value.magnitude_words() {
        writer.u64(*word);
    }
    Ok(())
}

fn decode_integer(reader: &mut Reader<'_>) -> Result<BigInt, CodecError> {
    let negative = reader.boolean()?;
    let count = reader.count()?;
    let mut words = Vec::new();
    for _ in 0..count {
        words.push(reader.u64()?);
    }
    BigInt::from_canonical_words(negative, words).map_err(CodecError::MalformedStructuralFoundation)
}

pub(super) fn encode(writer: &mut Writer, conditions: &ScalarConditions) -> Result<(), CodecError> {
    writer.len("scalar range conditions", conditions.ranges().len())?;
    for condition in conditions.ranges() {
        let ScalarRangeCondition::Range {
            subject,
            requirement,
            ..
        } = condition
        else {
            return Err(CodecError::MalformedStructuralFoundation(
                "scalar condition is absent",
            ));
        };
        match subject {
            ScalarConditionSubject::Module => writer.u8(1),
            ScalarConditionSubject::Declaration(declaration) => {
                writer.u8(2);
                writer.u32(declaration.0);
            }
            ScalarConditionSubject::StructuralType(subject) => {
                writer.u8(3);
                writer.id(*subject);
            }
            ScalarConditionSubject::Application(application) => {
                writer.u8(4);
                writer.id(*application);
            }
        }
        match requirement {
            ScalarRangeRequirement::Fixed { minimum, maximum } => {
                writer.u8(1);
                encode_integer(writer, minimum)?;
                encode_integer(writer, maximum)?;
            }
            ScalarRangeRequirement::AddressModel => writer.u8(2),
        }
        let steps = conditions
            .steps(condition)
            .map_err(CodecError::MalformedStructuralFoundation)?;
        writer.len("scalar bound steps", steps.len())?;
        for step in steps {
            match step {
                IntegerBoundStep::Missing => {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "scalar condition contains an absent step",
                    ));
                }
                IntegerBoundStep::Bounds { low, high } => {
                    writer.u8(1);
                    encode_integer(writer, low)?;
                    encode_integer(writer, high)?;
                }
                IntegerBoundStep::Observation(observation) => {
                    writer.u8(2);
                    match observation {
                        ScalarRangeObservation::Missing => {
                            return Err(CodecError::MalformedStructuralFoundation(
                                "scalar condition contains an absent observation",
                            ));
                        }
                        ScalarRangeObservation::Literal(value) => {
                            writer.u8(1);
                            encode_integer(writer, value)?;
                        }
                        ScalarRangeObservation::Application(application) => {
                            writer.u8(2);
                            writer.id(*application);
                        }
                        ScalarRangeObservation::EntryResult {
                            application,
                            machine,
                            result,
                            scalar_type,
                        } => {
                            writer.u8(3);
                            writer.id(*application);
                            writer.id(*machine);
                            writer.id(*result);
                            encode_scalar_type(writer, *scalar_type);
                        }
                    }
                }
                IntegerBoundStep::Add => writer.u8(3),
                IntegerBoundStep::Subtract => writer.u8(4),
                IntegerBoundStep::Multiply => writer.u8(5),
                IntegerBoundStep::Union => writer.u8(6),
                IntegerBoundStep::Interval => writer.u8(7),
                IntegerBoundStep::Intersect => writer.u8(8),
            }
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<ScalarConditions, CodecError> {
    let count = reader.count()?;
    let mut conditions = ScalarConditions::default();
    for _ in 0..count {
        let subject = match reader.u8()? {
            1 => ScalarConditionSubject::Module,
            2 => {
                ScalarConditionSubject::Declaration(terminal_psi::ProofDeclarationId(reader.u32()?))
            }
            3 => ScalarConditionSubject::StructuralType(reader.id("StructuralTypeId")?),
            4 => ScalarConditionSubject::Application(reader.id("ScalarApplicationId")?),
            tag => return Err(CodecError::InvalidTag("ScalarConditionSubject", tag)),
        };
        let requirement = match reader.u8()? {
            1 => ScalarRangeRequirement::Fixed {
                minimum: decode_integer(reader)?,
                maximum: decode_integer(reader)?,
            },
            2 => ScalarRangeRequirement::AddressModel,
            tag => return Err(CodecError::InvalidTag("ScalarRangeRequirement", tag)),
        };
        let count = reader.count()?;
        let mut steps = Vec::new();
        for _ in 0..count {
            let step = match reader.u8()? {
                1 => IntegerBoundStep::Bounds {
                    low: decode_integer(reader)?,
                    high: decode_integer(reader)?,
                },
                2 => IntegerBoundStep::Observation(match reader.u8()? {
                    1 => ScalarRangeObservation::Literal(decode_integer(reader)?),
                    2 => ScalarRangeObservation::Application(reader.id("ScalarApplicationId")?),
                    3 => ScalarRangeObservation::EntryResult {
                        application: reader.id("ScalarApplicationId")?,
                        machine: reader.id("MachineId")?,
                        result: reader.id("ValueId")?,
                        scalar_type: decode_scalar_type(reader)?,
                    },
                    tag => return Err(CodecError::InvalidTag("ScalarRangeObservation", tag)),
                }),
                3 => IntegerBoundStep::Add,
                4 => IntegerBoundStep::Subtract,
                5 => IntegerBoundStep::Multiply,
                6 => IntegerBoundStep::Union,
                7 => IntegerBoundStep::Interval,
                8 => IntegerBoundStep::Intersect,
                tag => return Err(CodecError::InvalidTag("IntegerBoundStep", tag)),
            };
            steps.push(step);
        }
        conditions
            .retain_range(subject, requirement, steps)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    Ok(conditions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, MachineId, ScalarApplicationId, ScalarType, StructuralTypeId,
        ValueId,
    };
    use terminal_psi::ProofDeclarationId;

    #[test]
    fn scalar_conditions_round_trip_subjects_observations_and_bound_operations() {
        let application = ScalarApplicationId::new(3).unwrap();
        let machine = MachineId::new(7).unwrap();
        let entry_result = ScalarRangeObservation::EntryResult {
            application,
            machine,
            result: ValueId::new(11).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        };
        let wide = BigInt::from_u64(1).shl_bits(260);
        let mut steps = vec![IntegerBoundStep::Observation(
            ScalarRangeObservation::Application(application),
        )];
        for operation in [
            IntegerBoundStep::Add,
            IntegerBoundStep::Subtract,
            IntegerBoundStep::Multiply,
            IntegerBoundStep::Union,
            IntegerBoundStep::Interval,
            IntegerBoundStep::Intersect,
        ] {
            steps.push(IntegerBoundStep::Observation(entry_result.clone()));
            steps.push(operation);
        }
        steps.extend([
            IntegerBoundStep::Bounds {
                low: wide.negate(),
                high: wide.clone(),
            },
            IntegerBoundStep::Intersect,
            IntegerBoundStep::Observation(ScalarRangeObservation::Literal(BigInt::from_u64(5))),
            IntegerBoundStep::Union,
        ]);
        let mut conditions = ScalarConditions::default();
        for subject in [
            ScalarConditionSubject::Module,
            ScalarConditionSubject::Declaration(ProofDeclarationId(7)),
            ScalarConditionSubject::StructuralType(StructuralTypeId::new(13).unwrap()),
            ScalarConditionSubject::Application(application),
        ] {
            for requirement in [
                ScalarRangeRequirement::AddressModel,
                ScalarRangeRequirement::Fixed {
                    minimum: wide.negate(),
                    maximum: wide.clone(),
                },
            ] {
                conditions
                    .retain_range(subject, requirement, steps.clone())
                    .unwrap();
            }
        }
        let mut writer = Writer::default();
        encode(&mut writer, &conditions).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        let decoded = decode(&mut reader).unwrap();
        assert_eq!(decoded, conditions);
        assert_eq!(reader.remaining(), 0);
        let mut writer = Writer::default();
        encode(&mut writer, &decoded).unwrap();
        assert_eq!(writer.finish(), bytes);
    }

    #[test]
    fn scalar_conditions_reject_malformed_bound_stack() {
        let mut writer = Writer::default();
        writer.u32(1); // One condition.
        writer.u8(1); // Module subject.
        writer.u8(2); // Address-model requirement.
        writer.u32(1); // One step.
        writer.u8(3); // Add without operands.
        assert!(matches!(
            decode(&mut Reader::new(&writer.finish())),
            Err(CodecError::MalformedStructuralFoundation(
                "scalar condition has missing bound operands"
            ))
        ));
    }

    #[test]
    fn scalar_conditions_reject_noncanonical_and_truncated_integers() {
        for (negative, words) in [(true, vec![]), (false, vec![0]), (false, vec![9, 0])] {
            let mut writer = Writer::default();
            writer.boolean(negative);
            writer.len("words", words.len()).unwrap();
            for word in words {
                writer.u64(word);
            }
            assert!(matches!(
                decode_integer(&mut Reader::new(&writer.finish())),
                Err(CodecError::MalformedStructuralFoundation(
                    "integer magnitude is not canonical"
                ))
            ));
        }
        let mut writer = Writer::default();
        writer.boolean(false);
        writer.u32(u32::MAX);
        assert!(matches!(
            decode_integer(&mut Reader::new(&writer.finish())),
            Err(CodecError::UnexpectedEnd)
        ));
    }
}
