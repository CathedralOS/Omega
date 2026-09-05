//! Structural contract terms; recovery grants no proof or invocation authority.

use super::Error;
use super::identity::{nominal, operator_coordinate, type_identity};
use super::reader::Reader;
use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewAtomicLoadOrdering,
    PackageReviewByteSequencePredicate, PackageReviewCastForm,
    PackageReviewCollectionViewOperation, PackageReviewConstructorField,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractEvidenceArgument, PackageReviewContractEvidenceTerm,
    PackageReviewContractExpression, PackageReviewContractKind,
    PackageReviewContractOperatorMeaning, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewFloatLiteral, PackageReviewReferenceAccess,
};
use psi_symbols::BuiltinFunction;

pub(super) fn expression(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractExpression, Error> {
    reader.nested(|reader| {
        use PackageReviewContractExpression as Expression;
        Ok(match reader.byte()? {
            0 => Expression::Boolean(reader.boolean()?),
            1 => Expression::Integer(reader.string()?),
            2 => Expression::Parameter(reader.u32()?),
            3 => Expression::Result,
            4 => Expression::GenericBinder(reader.u32()?),
            5 => Expression::Nominal(nominal(reader)?),
            6 => Expression::Binary {
                meaning: operator_meaning(reader)?,
                operator: binary_operator(reader)?,
                left: reader.boxed(expression)?,
                right: reader.boxed(expression)?,
            },
            7 => Expression::Unary {
                operator: match reader.byte()? {
                    0 => PackageReviewContractUnaryOperator::BitwiseNot,
                    1 => PackageReviewContractUnaryOperator::LogicalNot,
                    _ => return Err(Error::InvalidTag),
                },
                operand: reader.boxed(expression)?,
            },
            8 => Expression::Member {
                receiver: reader.boxed(expression)?,
                member: nominal(reader)?,
                case_variant: reader.option(nominal)?,
            },
            9 => Expression::Cast {
                value: reader.boxed(expression)?,
                target: type_identity(reader)?,
                arithmetic_domain: match reader.byte()? {
                    0 => PackageReviewArithmeticDomain::Exact,
                    1 => PackageReviewArithmeticDomain::Wrapping,
                    2 => PackageReviewArithmeticDomain::Saturating,
                    3 => PackageReviewArithmeticDomain::Trapping,
                    _ => return Err(Error::InvalidTag),
                },
                semantic_domain: reader.option(nominal)?,
                semantic_domain_arguments: reader.sequence(8, type_identity)?,
                form: match reader.byte()? {
                    0 => PackageReviewCastForm::Value,
                    1 => PackageReviewCastForm::RecastShared,
                    2 => PackageReviewCastForm::RecastMutable,
                    _ => return Err(Error::InvalidTag),
                },
            },
            10 => Expression::DomainSubject,
            11 => Expression::Call {
                receiver: reader.option(|reader| reader.boxed(expression))?,
                target: call_target(reader)?,
                static_arguments: reader.sequence(1, static_argument)?,
                evidence_arguments: reader.sequence(96, evidence_argument)?,
                arguments: reader.sequence(1, expression)?,
            },
            12 => Expression::ByteSequence(reader.bytes()?),
            13 => Expression::ZeroValue(type_identity(reader)?),
            14 => Expression::Array(reader.sequence(1, expression)?),
            15 => Expression::Constructor {
                data: nominal(reader)?,
                case: reader.option(nominal)?,
                fields: reader.sequence(42, |reader| {
                    Ok(PackageReviewConstructorField {
                        field: nominal(reader)?,
                        value: expression(reader)?,
                    })
                })?,
            },
            16 => Expression::Indexed {
                meaning: operator_meaning(reader)?,
                collection: reader.boxed(expression)?,
                index: reader.boxed(expression)?,
            },
            17 => Expression::Range {
                start: reader.option(|reader| reader.boxed(expression))?,
                end: reader.option(|reader| reader.boxed(expression))?,
                end_inclusive: reader.boolean()?,
            },
            18 => Expression::CollectionLength {
                collection: reader.boxed(expression)?,
            },
            19 => Expression::Float(match reader.byte()? {
                0 => PackageReviewFloatLiteral::F32(reader.u32()?),
                1 => PackageReviewFloatLiteral::F64(reader.u64()?),
                _ => return Err(Error::InvalidTag),
            }),
            20 => Expression::Reference {
                access: match reader.byte()? {
                    0 => PackageReviewReferenceAccess::Shared,
                    1 => PackageReviewReferenceAccess::Mutable,
                    2 => PackageReviewReferenceAccess::WriteOnly,
                    _ => return Err(Error::InvalidTag),
                },
                target: reader.boxed(expression)?,
            },
            21 => {
                let ordering = match reader.byte()? {
                    0 => PackageReviewAtomicLoadOrdering::NoOrdering,
                    1 => PackageReviewAtomicLoadOrdering::Receive,
                    2 => PackageReviewAtomicLoadOrdering::GlobalOrder,
                    _ => return Err(Error::InvalidTag),
                };
                Expression::AtomicLoad {
                    value: reader.boxed(expression)?,
                    ordering,
                }
            }
            _ => return Err(Error::InvalidTag),
        })
    })
}

fn operator_meaning(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractOperatorMeaning, Error> {
    match reader.byte()? {
        0 => Ok(PackageReviewContractOperatorMeaning::Builtin),
        1 => Ok(PackageReviewContractOperatorMeaning::Declared(
            operator_coordinate(reader)?,
        )),
        _ => Err(Error::InvalidTag),
    }
}

fn binary_operator(reader: &mut Reader<'_>) -> Result<PackageReviewContractBinaryOperator, Error> {
    use PackageReviewContractBinaryOperator as Operator;
    Ok(match reader.byte()? {
        0 => Operator::Add,
        1 => Operator::And,
        2 => Operator::BitwiseAnd,
        3 => Operator::BitwiseOr,
        4 => Operator::BitwiseXor,
        5 => Operator::Divide,
        6 => Operator::Equal,
        7 => Operator::Greater,
        8 => Operator::GreaterOrEqual,
        9 => Operator::Less,
        10 => Operator::LessOrEqual,
        11 => Operator::Modulo,
        12 => Operator::Multiply,
        13 => Operator::NotEqual,
        14 => Operator::Or,
        15 => Operator::ShiftLeft,
        16 => Operator::ShiftRight,
        17 => Operator::Subtract,
        _ => return Err(Error::InvalidTag),
    })
}

fn call_target(reader: &mut Reader<'_>) -> Result<PackageReviewContractCallTarget, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewContractCallTarget::Nominal(nominal(reader)?),
        1 => PackageReviewContractCallTarget::ByteSequencePredicate(match reader.byte()? {
            0 => PackageReviewByteSequencePredicate::ValidUtf8,
            1 => PackageReviewByteSequencePredicate::NoNul,
            2 => PackageReviewByteSequencePredicate::AsciiOnly,
            3 => PackageReviewByteSequencePredicate::NonEmpty,
            _ => return Err(Error::InvalidTag),
        }),
        2 => PackageReviewContractCallTarget::BuiltinFunction(
            BuiltinFunction::from_ordinal(usize::from(reader.u16()?)).ok_or(Error::InvalidTag)?,
        ),
        3 => PackageReviewContractCallTarget::CollectionView(match reader.byte()? {
            0 => PackageReviewCollectionViewOperation::SharedSlice,
            1 => PackageReviewCollectionViewOperation::MutableSlice,
            2 => PackageReviewCollectionViewOperation::TextView,
            3 => PackageReviewCollectionViewOperation::Bytes,
            _ => return Err(Error::InvalidTag),
        }),
        _ => return Err(Error::InvalidTag),
    })
}

fn evidence_argument(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractEvidenceArgument, Error> {
    Ok(PackageReviewContractEvidenceArgument {
        lane_position: reader.u32()?,
        source: evidence_term(reader)?,
        parameter: evidence_term(reader)?,
    })
}

fn evidence_term(reader: &mut Reader<'_>) -> Result<PackageReviewContractEvidenceTerm, Error> {
    Ok(PackageReviewContractEvidenceTerm {
        owner: nominal(reader)?,
        kind: match reader.byte()? {
            0 => PackageReviewContractKind::Requires,
            1 => PackageReviewContractKind::Ensures,
            _ => return Err(Error::InvalidTag),
        },
        lane_position: reader.u32()?,
    })
}

pub(super) fn static_argument(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewContractStaticArgument, Error> {
    reader.nested(|reader| {
        use PackageReviewContractStaticArgument as Argument;
        Ok(match reader.byte()? {
            0 => Argument::Type(type_identity(reader)?),
            1 => Argument::GenericType {
                base: type_identity(reader)?,
                lifetime_arguments: reader.sequence(4, Reader::u32)?,
                arguments: reader.sequence(1, static_argument)?,
            },
            2 => Argument::ConstInteger(reader.string()?),
            3 => Argument::GenericMachineBinder(reader.u32()?),
            4 => Argument::ConcreteMachine(nominal(reader)?),
            5 => Argument::GenericTypeBinder(reader.u32()?),
            6 => Argument::GenericConstBinder(reader.u32()?),
            7 => Argument::ConstBoolean(reader.boolean()?),
            8 => Argument::ConformanceApplication {
                declaration: nominal(reader)?,
                arguments: reader.sequence(1, static_argument)?,
                subject: reader.boxed(static_argument)?,
                trait_identity: nominal(reader)?,
                trait_arguments: reader.sequence(8, type_identity)?,
            },
            9 => Argument::ConstStructured {
                declared_type: type_identity(reader)?,
                canonical_value_encoding: reader.string()?,
            },
            _ => return Err(Error::InvalidTag),
        })
    })
}

#[cfg(test)]
mod tests;
