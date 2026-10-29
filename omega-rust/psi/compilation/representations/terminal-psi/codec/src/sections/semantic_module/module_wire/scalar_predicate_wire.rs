//! Captured Boolean graphs use preceding-node wire coordinates, not arena generations.

use super::super::{
    CodecError,
    wire::{Reader, Writer},
};
use super::scalar_condition_wire::{
    decode_integer, decode_observation, decode_subject, encode_integer, encode_observation,
    encode_subject,
};
use arena::{Arena, Handle};
use numerics::scalar_predicates::{PredicateNode, PredicateOperator, ScalarValue};
use terminal_psi::{ScalarConditions, ScalarPredicateNode};

fn coordinate(
    references: &[Handle<ScalarPredicateNode>],
    reference: Handle<ScalarPredicateNode>,
) -> Result<u32, CodecError> {
    references
        .iter()
        .position(|candidate| *candidate == reference)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or(CodecError::MalformedStructuralFoundation(
            "scalar predicate lost a preceding child",
        ))
}

fn reference(
    reader: &mut Reader<'_>,
    references: &[Handle<ScalarPredicateNode>],
) -> Result<Handle<ScalarPredicateNode>, CodecError> {
    references.get(reader.u32()? as usize).copied().ok_or(
        CodecError::MalformedStructuralFoundation("scalar predicate lost a preceding child"),
    )
}

pub(super) fn encode(writer: &mut Writer, conditions: &ScalarConditions) -> Result<(), CodecError> {
    writer.len("scalar predicates", conditions.predicates().len())?;
    for condition in conditions.predicates() {
        encode_subject(writer, condition.subject);
        writer.string("scalar predicate description", &condition.description)?;
        writer.len("scalar predicate nodes", condition.nodes().len())?;
        let mut references = Vec::new();
        for (handle, node) in condition.nodes().iter() {
            match &node.0 {
                PredicateNode::Missing | PredicateNode::Value(ScalarValue::Unknown) => {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "scalar predicate contains an absent value",
                    ));
                }
                PredicateNode::Value(ScalarValue::Integer(value)) => {
                    writer.u8(1);
                    encode_integer(writer, value)?;
                }
                PredicateNode::Value(ScalarValue::Boolean(value)) => {
                    writer.u8(2);
                    writer.boolean(*value);
                }
                PredicateNode::Leaf(observation) => {
                    writer.u8(3);
                    encode_observation(writer, observation)?;
                }
                PredicateNode::Forward(child) | PredicateNode::Not(child) => {
                    writer.u8(if matches!(node.0, PredicateNode::Forward(_)) {
                        4
                    } else {
                        5
                    });
                    writer.u32(coordinate(&references, *child)?);
                }
                PredicateNode::Binary {
                    operator,
                    left,
                    right,
                } => {
                    writer.u8(6);
                    writer.u8(match operator {
                        PredicateOperator::And => 1,
                        PredicateOperator::Or => 2,
                        PredicateOperator::Equal => 3,
                        PredicateOperator::NotEqual => 4,
                        PredicateOperator::Less => 5,
                        PredicateOperator::LessOrEqual => 6,
                        PredicateOperator::Greater => 7,
                        PredicateOperator::GreaterOrEqual => 8,
                    });
                    writer.u32(coordinate(&references, *left)?);
                    writer.u32(coordinate(&references, *right)?);
                }
            }
            references.push(handle);
        }
        writer.u32(coordinate(&references, condition.root())?);
    }
    Ok(())
}

pub(super) fn decode(
    reader: &mut Reader<'_>,
    conditions: &mut ScalarConditions,
) -> Result<(), CodecError> {
    let count = reader.count()?;
    for _ in 0..count {
        let subject = decode_subject(reader)?;
        let description = reader.string("scalar predicate description")?;
        let count = reader.count()?;
        let mut nodes = Arena::new();
        let mut references = Vec::new();
        for _ in 0..count {
            let node = match reader.u8()? {
                1 => PredicateNode::Value(ScalarValue::Integer(decode_integer(reader)?)),
                2 => PredicateNode::Value(ScalarValue::Boolean(reader.boolean()?)),
                3 => PredicateNode::Leaf(decode_observation(reader)?),
                4 => PredicateNode::Forward(reference(reader, &references)?),
                5 => PredicateNode::Not(reference(reader, &references)?),
                6 => {
                    let operator = match reader.u8()? {
                        1 => PredicateOperator::And,
                        2 => PredicateOperator::Or,
                        3 => PredicateOperator::Equal,
                        4 => PredicateOperator::NotEqual,
                        5 => PredicateOperator::Less,
                        6 => PredicateOperator::LessOrEqual,
                        7 => PredicateOperator::Greater,
                        8 => PredicateOperator::GreaterOrEqual,
                        tag => return Err(CodecError::InvalidTag("PredicateOperator", tag)),
                    };
                    PredicateNode::Binary {
                        operator,
                        left: reference(reader, &references)?,
                        right: reference(reader, &references)?,
                    }
                }
                tag => return Err(CodecError::InvalidTag("ScalarPredicateNode", tag)),
            };
            references.push(nodes.insert(ScalarPredicateNode(node)));
        }
        let root = reference(reader, &references)?;
        conditions
            .retain_predicate(subject, description, nodes, root)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::bignum::BigInt;
    use semantic_vocabulary::ScalarApplicationId;
    use terminal_psi::{ScalarConditionSubject, ScalarRangeObservation};

    #[test]
    fn scalar_predicate_round_trip_retains_both_equality_operands_and_short_circuiting() {
        let first = ScalarApplicationId::new(13).unwrap();
        let second = ScalarApplicationId::new(19).unwrap();
        let mut nodes = Arena::new();
        let left = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Application(first),
        )));
        let right = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Application(second),
        )));
        let equality = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
            operator: PredicateOperator::Equal,
            left,
            right,
        }));
        let mut conditions = ScalarConditions::default();
        conditions
            .retain_predicate(
                ScalarConditionSubject::Module,
                "both recipes agree".into(),
                nodes,
                equality,
            )
            .unwrap();
        let mut nodes = Arena::new();
        let known = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
            ScalarValue::Boolean(true),
        )));
        let unknown = nodes.insert(ScalarPredicateNode(PredicateNode::Leaf(
            ScalarRangeObservation::Application(second),
        )));
        let bound = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
            ScalarValue::Integer(BigInt::from_u64(24)),
        )));
        let comparison = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
            operator: PredicateOperator::Equal,
            left: unknown,
            right: bound,
        }));
        let root = nodes.insert(ScalarPredicateNode(PredicateNode::Binary {
            operator: PredicateOperator::Or,
            left: known,
            right: comparison,
        }));
        conditions
            .retain_predicate(
                ScalarConditionSubject::Module,
                "selected disjunction".into(),
                nodes,
                root,
            )
            .unwrap();
        let mut writer = Writer::default();
        encode(&mut writer, &conditions).unwrap();
        let bytes = writer.finish();
        drop(conditions);
        let mut decoded = ScalarConditions::default();
        let mut reader = Reader::new(&bytes);
        decode(&mut reader, &mut decoded).unwrap();
        assert_eq!(reader.remaining(), 0);
        assert_eq!(decoded.observations().count(), 3);
        for (second_value, expected) in [
            (Some(24), Some(true)),
            (Some(16), Some(false)),
            (None, None),
        ] {
            let mut observed = Vec::new();
            let result = decoded.predicates()[0].evaluate(&mut |observation| {
                let ScalarRangeObservation::Application(application) = observation else {
                    panic!("expected application");
                };
                observed.push(application);
                let value = if application == first {
                    Some(24)
                } else {
                    second_value
                }?;
                Some(ScalarValue::Integer(BigInt::from_u64(value)))
            });
            assert_eq!(observed, vec![first, second]);
            assert_eq!(result, expected.map(ScalarValue::Boolean));
        }
        assert_eq!(
            decoded.predicates()[1].evaluate(&mut |_| panic!("skipped observation")),
            Some(ScalarValue::Boolean(true))
        );
        let mut writer = Writer::default();
        encode(&mut writer, &decoded).unwrap();
        assert_eq!(writer.finish(), bytes);
    }

    #[test]
    fn scalar_predicate_codec_rejects_forward_edges_and_missing_roots() {
        for forward in [false, true] {
            let mut writer = Writer::default();
            writer.u32(1);
            encode_subject(&mut writer, ScalarConditionSubject::Module);
            writer.string("description", "malformed graph").unwrap();
            writer.u32(1);
            if forward {
                writer.u8(4);
                writer.u32(0);
            } else {
                writer.u8(2);
                writer.boolean(true);
                writer.u32(1);
            }
            assert!(matches!(
                decode(
                    &mut Reader::new(&writer.finish()),
                    &mut ScalarConditions::default()
                ),
                Err(CodecError::MalformedStructuralFoundation(
                    "scalar predicate lost a preceding child"
                ))
            ));
        }
    }

    #[test]
    fn scalar_predicate_codec_rejects_ill_typed_skipped_operands() {
        let mut writer = Writer::default();
        writer.u32(1);
        encode_subject(&mut writer, ScalarConditionSubject::Module);
        writer
            .string("description", "true or integer is not formed")
            .unwrap();
        writer.u32(3);
        writer.u8(2);
        writer.boolean(true);
        writer.u8(1);
        encode_integer(&mut writer, &BigInt::from_u64(7)).unwrap();
        writer.u8(6);
        writer.u8(2);
        writer.u32(0);
        writer.u32(1);
        writer.u32(2);
        assert!(matches!(
            decode(
                &mut Reader::new(&writer.finish()),
                &mut ScalarConditions::default()
            ),
            Err(CodecError::MalformedStructuralFoundation(
                "scalar predicate operator has incompatible operand kinds"
            ))
        ));
    }
}
