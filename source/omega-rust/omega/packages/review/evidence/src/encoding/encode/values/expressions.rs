use super::super::declarations::encode_type_identity;
use super::super::encoder::Encoder;
use crate::encoding::PackageReviewEncodingError;
use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCastForm,
    PackageReviewCollectionViewOperation, PackageReviewContractBinaryOperator,
    PackageReviewContractCallTarget, PackageReviewContractEvidenceArgument,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewFloatLiteral, PackageReviewReferenceAccess,
};

use super::declarations::encode_operator_coordinate;
use super::identity::encode_nominal;

pub(crate) fn encode_contract_expression(
    encoder: &mut Encoder,
    expression: &PackageReviewContractExpression,
) -> Result<(), PackageReviewEncodingError> {
    match expression {
        PackageReviewContractExpression::Boolean(value) => {
            encoder.byte(0);
            encoder.boolean(*value);
        }
        PackageReviewContractExpression::Integer(value) => {
            encoder.byte(1);
            encoder.string(value)?;
        }
        PackageReviewContractExpression::Float(value) => {
            encoder.byte(19);
            match value {
                PackageReviewFloatLiteral::F32(bits) => {
                    encoder.byte(0);
                    encoder.u32(*bits);
                }
                PackageReviewFloatLiteral::F64(bits) => {
                    encoder.byte(1);
                    encoder.u64(*bits);
                }
            }
        }
        PackageReviewContractExpression::Array(values) => {
            encoder.byte(14);
            encoder.sequence(values, encode_contract_expression)?;
        }
        PackageReviewContractExpression::Constructor { data, case, fields } => {
            encoder.byte(15);
            encode_nominal(encoder, data)?;
            encoder.option(case.as_ref(), encode_nominal)?;
            encoder.sequence(fields, |encoder, field| {
                encode_nominal(encoder, &field.field)?;
                encode_contract_expression(encoder, &field.value)
            })?;
        }
        PackageReviewContractExpression::Indexed {
            meaning,
            collection,
            index,
        } => {
            encoder.byte(16);
            encode_contract_operator_meaning(encoder, meaning)?;
            encode_contract_expression(encoder, collection)?;
            encode_contract_expression(encoder, index)?;
        }
        PackageReviewContractExpression::Range {
            start,
            end,
            end_inclusive,
        } => {
            encoder.byte(17);
            encoder.option(start.as_deref(), encode_contract_expression)?;
            encoder.option(end.as_deref(), encode_contract_expression)?;
            encoder.boolean(*end_inclusive);
        }
        PackageReviewContractExpression::ByteSequence(value) => {
            encoder.byte(12);
            encoder.bytes(value)?;
        }
        PackageReviewContractExpression::DomainSubject => encoder.byte(10),
        PackageReviewContractExpression::Parameter(position) => {
            encoder.byte(2);
            encoder.u32(*position);
        }
        PackageReviewContractExpression::Result => encoder.byte(3),
        PackageReviewContractExpression::GenericBinder(position) => {
            encoder.byte(4);
            encoder.u32(*position);
        }
        PackageReviewContractExpression::Nominal(identity) => {
            encoder.byte(5);
            encode_nominal(encoder, identity)?;
        }
        PackageReviewContractExpression::Reference { access, target } => {
            encoder.byte(20);
            encoder.byte(match access {
                PackageReviewReferenceAccess::Shared => 0,
                PackageReviewReferenceAccess::Mutable => 1,
                PackageReviewReferenceAccess::WriteOnly => 2,
            });
            encode_contract_expression(encoder, target)?;
        }
        PackageReviewContractExpression::AtomicLoad { value, ordering } => {
            encoder.byte(21);
            encoder.byte(match ordering {
                PackageReviewAtomicLoadOrdering::NoOrdering => 0,
                PackageReviewAtomicLoadOrdering::Receive => 1,
                PackageReviewAtomicLoadOrdering::GlobalOrder => 2,
            });
            encode_contract_expression(encoder, value)?;
        }
        PackageReviewContractExpression::ZeroValue(type_identity) => {
            encoder.byte(13);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewContractExpression::CollectionLength { collection } => {
            encoder.byte(18);
            encode_contract_expression(encoder, collection)?;
        }
        PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } => {
            encoder.byte(8);
            encode_contract_expression(encoder, receiver)?;
            encode_nominal(encoder, member)?;
            encoder.option(case_variant.as_ref(), encode_nominal)?;
        }
        PackageReviewContractExpression::Cast {
            value,
            target,
            arithmetic_domain,
            semantic_domain,
            semantic_domain_arguments,
            form,
        } => {
            encoder.byte(9);
            encode_contract_expression(encoder, value)?;
            encode_type_identity(encoder, target)?;
            encoder.byte(match arithmetic_domain {
                PackageReviewArithmeticDomain::Exact => 0,
                PackageReviewArithmeticDomain::Wrapping => 1,
                PackageReviewArithmeticDomain::Saturating => 2,
                PackageReviewArithmeticDomain::Trapping => 3,
            });
            encoder.option(semantic_domain.as_ref(), encode_nominal)?;
            encoder.sequence(semantic_domain_arguments, encode_type_identity)?;
            encoder.byte(match form {
                PackageReviewCastForm::Value => 0,
                PackageReviewCastForm::RecastShared => 1,
                PackageReviewCastForm::RecastMutable => 2,
            });
        }
        PackageReviewContractExpression::Call {
            receiver,
            target,
            static_arguments,
            evidence_arguments,
            arguments,
        } => {
            encoder.byte(11);
            encoder.option(receiver.as_deref(), encode_contract_expression)?;
            match target {
                PackageReviewContractCallTarget::Nominal(identity) => {
                    encoder.byte(0);
                    encode_nominal(encoder, identity)?;
                }
                PackageReviewContractCallTarget::BuiltinFunction(function) => {
                    encoder.byte(2);
                    encoder.u16(u16::try_from(function.ordinal()).map_err(|_| {
                        PackageReviewEncodingError::new(
                            "compiler builtin-function ordinal exceeds the portable encoding range",
                        )
                    })?);
                }
                PackageReviewContractCallTarget::ByteSequencePredicate(predicate) => {
                    encoder.byte(1);
                    encoder.byte(match predicate {
                        PackageReviewByteSequencePredicate::ValidUtf8 => 0,
                        PackageReviewByteSequencePredicate::NoNul => 1,
                        PackageReviewByteSequencePredicate::AsciiOnly => 2,
                        PackageReviewByteSequencePredicate::NonEmpty => 3,
                    });
                }
                PackageReviewContractCallTarget::CollectionView(operation) => {
                    encoder.byte(3);
                    encoder.byte(match operation {
                        PackageReviewCollectionViewOperation::SharedSlice => 0,
                        PackageReviewCollectionViewOperation::MutableSlice => 1,
                        PackageReviewCollectionViewOperation::TextView => 2,
                        PackageReviewCollectionViewOperation::Bytes => 3,
                    });
                }
            }
            encoder.sequence(static_arguments, encode_contract_static_argument)?;
            encoder.sequence(evidence_arguments, encode_contract_evidence_argument)?;
            encoder.sequence(arguments, encode_contract_expression)?;
        }
        PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        } => {
            encoder.byte(6);
            encode_contract_operator_meaning(encoder, meaning)?;
            encoder.byte(match operator {
                PackageReviewContractBinaryOperator::Add => 0,
                PackageReviewContractBinaryOperator::And => 1,
                PackageReviewContractBinaryOperator::BitwiseAnd => 2,
                PackageReviewContractBinaryOperator::BitwiseOr => 3,
                PackageReviewContractBinaryOperator::BitwiseXor => 4,
                PackageReviewContractBinaryOperator::Divide => 5,
                PackageReviewContractBinaryOperator::Equal => 6,
                PackageReviewContractBinaryOperator::Greater => 7,
                PackageReviewContractBinaryOperator::GreaterOrEqual => 8,
                PackageReviewContractBinaryOperator::Less => 9,
                PackageReviewContractBinaryOperator::LessOrEqual => 10,
                PackageReviewContractBinaryOperator::Modulo => 11,
                PackageReviewContractBinaryOperator::Multiply => 12,
                PackageReviewContractBinaryOperator::NotEqual => 13,
                PackageReviewContractBinaryOperator::Or => 14,
                PackageReviewContractBinaryOperator::ShiftLeft => 15,
                PackageReviewContractBinaryOperator::ShiftRight => 16,
                PackageReviewContractBinaryOperator::Subtract => 17,
            });
            encode_contract_expression(encoder, left)?;
            encode_contract_expression(encoder, right)?;
        }
        PackageReviewContractExpression::Unary { operator, operand } => {
            encoder.byte(7);
            encoder.byte(match operator {
                PackageReviewContractUnaryOperator::BitwiseNot => 0,
                PackageReviewContractUnaryOperator::LogicalNot => 1,
            });
            encode_contract_expression(encoder, operand)?;
        }
    }
    Ok(())
}

fn encode_contract_evidence_argument(
    encoder: &mut Encoder,
    argument: &PackageReviewContractEvidenceArgument,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u32(argument.lane_position());
    for term in [argument.source(), argument.parameter()] {
        encode_nominal(encoder, term.owner())?;
        encoder.byte(match term.kind() {
            PackageReviewContractKind::Requires => 0,
            PackageReviewContractKind::Ensures => 1,
        });
        encoder.u32(term.lane_position());
    }
    Ok(())
}

pub(crate) fn encode_contract_operator_meaning(
    encoder: &mut Encoder,
    meaning: &PackageReviewContractOperatorMeaning,
) -> Result<(), PackageReviewEncodingError> {
    match meaning {
        PackageReviewContractOperatorMeaning::Builtin => encoder.byte(0),
        PackageReviewContractOperatorMeaning::Declared(coordinate) => {
            encoder.byte(1);
            encode_operator_coordinate(encoder, coordinate)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_contract_static_argument(
    encoder: &mut Encoder,
    argument: &PackageReviewContractStaticArgument,
) -> Result<(), PackageReviewEncodingError> {
    match argument {
        PackageReviewContractStaticArgument::Type(identity) => {
            encoder.byte(0);
            encode_type_identity(encoder, identity)?;
        }
        PackageReviewContractStaticArgument::GenericTypeBinder(position) => {
            encoder.byte(5);
            encoder.u32(*position);
        }
        PackageReviewContractStaticArgument::GenericType {
            base,
            lifetime_arguments,
            arguments,
        } => {
            encoder.byte(1);
            encode_type_identity(encoder, base)?;
            encoder.sequence(lifetime_arguments, |encoder, argument| {
                encoder.u32(*argument);
                Ok(())
            })?;
            encoder.sequence(arguments, encode_contract_static_argument)?;
        }
        PackageReviewContractStaticArgument::ConstInteger(value) => {
            encoder.byte(2);
            encoder.string(value)?;
        }
        PackageReviewContractStaticArgument::ConstBoolean(value) => {
            encoder.byte(7);
            encoder.byte(u8::from(*value));
        }
        PackageReviewContractStaticArgument::GenericConstBinder(position) => {
            encoder.byte(6);
            encoder.u32(*position);
        }
        PackageReviewContractStaticArgument::GenericMachineBinder(position) => {
            encoder.byte(3);
            encoder.u32(*position);
        }
        PackageReviewContractStaticArgument::ConcreteMachine(identity) => {
            encoder.byte(4);
            encode_nominal(encoder, identity)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(argument: PackageReviewContractStaticArgument) -> Vec<u8> {
        let mut encoder = Encoder::bounded(16);
        encode_contract_static_argument(&mut encoder, &argument)
            .expect("encode contract static argument");
        encoder.finish().expect("bounded static-argument bytes")
    }

    #[test]
    fn boolean_static_arguments_have_a_distinct_closed_tag_and_value_byte() {
        assert_eq!(
            encoded(PackageReviewContractStaticArgument::ConstBoolean(false)),
            [7, 0]
        );
        assert_eq!(
            encoded(PackageReviewContractStaticArgument::ConstBoolean(true)),
            [7, 1]
        );
        assert_ne!(
            encoded(PackageReviewContractStaticArgument::ConstBoolean(true)),
            encoded(PackageReviewContractStaticArgument::ConstInteger(
                "1".to_owned()
            ))
        );
    }
}
