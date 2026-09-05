use super::*;
use crate::encoding::PackagePolicyRecoveryLimits;
use crate::encoding::encode::encoder::Encoder;
use crate::encoding::encode::{encode_contract_expression, encode_contract_static_argument};
use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewOperatorCoordinate,
    PackageReviewTypeIdentity,
};

fn identity(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(
            psi_core::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
        ),
        path: path.to_owned(),
    }
}

fn value_type() -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: "u64".to_owned(),
    }
}

fn operand() -> Box<PackageReviewContractExpression> {
    Box::new(PackageReviewContractExpression::Parameter(3))
}

fn meaning() -> PackageReviewContractOperatorMeaning {
    PackageReviewContractOperatorMeaning::Declared(PackageReviewOperatorCoordinate {
        identity: identity("Math::plus"),
        parameter_dispatch: "u64,u64".to_owned(),
        result_dispatch: "u64".to_owned(),
    })
}

fn encoded_expression(value: &PackageReviewContractExpression) -> Vec<u8> {
    let mut encoder = Encoder::bounded(1 << 20);
    encode_contract_expression(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}

fn recovered_expression(bytes: &[u8]) -> Result<PackageReviewContractExpression, Error> {
    let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
    let value = expression(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

fn encoded_static(value: &PackageReviewContractStaticArgument) -> Vec<u8> {
    let mut encoder = Encoder::bounded(1 << 20);
    encode_contract_static_argument(&mut encoder, value).unwrap();
    encoder.finish().unwrap()
}

fn recovered_static(bytes: &[u8]) -> Result<PackageReviewContractStaticArgument, Error> {
    let mut reader = Reader::new(bytes, PackagePolicyRecoveryLimits::default())?;
    let value = static_argument(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

fn static_arguments() -> Vec<PackageReviewContractStaticArgument> {
    use PackageReviewContractStaticArgument as Argument;
    vec![
        Argument::Type(value_type()),
        Argument::GenericType {
            base: value_type(),
            lifetime_arguments: vec![0, 2],
            arguments: vec![Argument::GenericTypeBinder(1), Argument::ConstBoolean(true)],
        },
        Argument::ConstInteger("-170141183460469231731687303715884105728".to_owned()),
        Argument::GenericMachineBinder(2),
        Argument::ConcreteMachine(identity("Handler::enter")),
        Argument::GenericTypeBinder(3),
        Argument::GenericConstBinder(4),
        Argument::ConstBoolean(false),
        Argument::ConformanceApplication {
            declaration: identity("CarrierImplements"),
            arguments: vec![Argument::ConstInteger("19".to_owned())],
            subject: Box::new(Argument::GenericType {
                base: value_type(),
                lifetime_arguments: vec![],
                arguments: vec![Argument::ConstBoolean(true)],
            }),
            trait_identity: identity("Callable"),
            trait_arguments: vec![value_type()],
        },
        Argument::ConstStructured {
            declared_type: value_type(),
            canonical_value_encoding: "retained structured value".to_owned(),
        },
    ]
}

fn evidence() -> PackageReviewContractEvidenceArgument {
    PackageReviewContractEvidenceArgument {
        lane_position: 7,
        source: PackageReviewContractEvidenceTerm {
            owner: identity("Source::enter"),
            kind: PackageReviewContractKind::Ensures,
            lane_position: 2,
        },
        parameter: PackageReviewContractEvidenceTerm {
            owner: identity("Destination::enter"),
            kind: PackageReviewContractKind::Requires,
            lane_position: 5,
        },
    }
}

fn call(target: PackageReviewContractCallTarget) -> PackageReviewContractExpression {
    PackageReviewContractExpression::Call {
        receiver: Some(operand()),
        target,
        static_arguments: static_arguments(),
        evidence_arguments: vec![evidence()],
        arguments: vec![PackageReviewContractExpression::ByteSequence(vec![
            0, 255, 128,
        ])],
    }
}

#[test]
fn every_static_argument_roundtrips_without_losing_nested_identity() {
    for value in static_arguments() {
        let bytes = encoded_static(&value);
        let recovered = recovered_static(&bytes).unwrap();
        assert_eq!(recovered, value);
        assert_eq!(encoded_static(&recovered), bytes);
    }
}

#[test]
fn every_expression_variant_roundtrips_exactly() {
    use PackageReviewContractExpression as Expression;
    let mut values = vec![
        Expression::Boolean(true),
        Expression::Integer("-123456789012345678901234567890".to_owned()),
        Expression::Parameter(3),
        Expression::Result,
        Expression::GenericBinder(4),
        Expression::Nominal(identity("Value")),
        Expression::Unary {
            operator: PackageReviewContractUnaryOperator::BitwiseNot,
            operand: operand(),
        },
        Expression::Unary {
            operator: PackageReviewContractUnaryOperator::LogicalNot,
            operand: operand(),
        },
        Expression::Member {
            receiver: operand(),
            member: identity("Shape::field"),
            case_variant: Some(identity("Shape::Case")),
        },
        Expression::DomainSubject,
        call(PackageReviewContractCallTarget::Nominal(identity(
            "Check::enter",
        ))),
        Expression::ByteSequence(vec![0, 255, 128]),
        Expression::ZeroValue(value_type()),
        Expression::Array(vec![Expression::Boolean(false), Expression::Result]),
        Expression::Constructor {
            data: identity("Pair"),
            case: Some(identity("Pair::Case")),
            fields: vec![PackageReviewConstructorField {
                field: identity("Pair::first"),
                value: Expression::Integer("19".to_owned()),
            }],
        },
        Expression::Indexed {
            meaning: meaning(),
            collection: operand(),
            index: operand(),
        },
        Expression::Range {
            start: None,
            end: Some(operand()),
            end_inclusive: false,
        },
        Expression::Range {
            start: Some(operand()),
            end: None,
            end_inclusive: true,
        },
        Expression::CollectionLength {
            collection: operand(),
        },
        Expression::Float(PackageReviewFloatLiteral::F32(0x8000_0000)),
        Expression::Float(PackageReviewFloatLiteral::F64(0x7ff8_0000_0000_0023)),
    ];
    for access in [
        PackageReviewReferenceAccess::Shared,
        PackageReviewReferenceAccess::Mutable,
        PackageReviewReferenceAccess::WriteOnly,
    ] {
        values.push(Expression::Reference {
            access,
            target: operand(),
        });
    }
    for ordering in [
        PackageReviewAtomicLoadOrdering::NoOrdering,
        PackageReviewAtomicLoadOrdering::Receive,
        PackageReviewAtomicLoadOrdering::GlobalOrder,
    ] {
        values.push(Expression::AtomicLoad {
            value: operand(),
            ordering,
        });
    }
    for arithmetic_domain in [
        PackageReviewArithmeticDomain::Exact,
        PackageReviewArithmeticDomain::Wrapping,
        PackageReviewArithmeticDomain::Saturating,
        PackageReviewArithmeticDomain::Trapping,
    ] {
        for form in [
            PackageReviewCastForm::Value,
            PackageReviewCastForm::RecastShared,
            PackageReviewCastForm::RecastMutable,
        ] {
            values.push(Expression::Cast {
                value: operand(),
                target: value_type(),
                arithmetic_domain,
                semantic_domain: Some(identity("Nonnegative")),
                semantic_domain_arguments: vec![value_type()],
                form,
            });
        }
    }
    use PackageReviewContractBinaryOperator as Operator;
    for operator in [
        Operator::Add,
        Operator::And,
        Operator::BitwiseAnd,
        Operator::BitwiseOr,
        Operator::BitwiseXor,
        Operator::Divide,
        Operator::Equal,
        Operator::Greater,
        Operator::GreaterOrEqual,
        Operator::Less,
        Operator::LessOrEqual,
        Operator::Modulo,
        Operator::Multiply,
        Operator::NotEqual,
        Operator::Or,
        Operator::ShiftLeft,
        Operator::ShiftRight,
        Operator::Subtract,
    ] {
        values.push(Expression::Binary {
            meaning: meaning(),
            operator,
            left: operand(),
            right: operand(),
        });
        values.push(Expression::Binary {
            meaning: PackageReviewContractOperatorMeaning::Builtin,
            operator,
            left: operand(),
            right: operand(),
        });
    }
    for value in values {
        let bytes = encoded_expression(&value);
        let recovered = recovered_expression(&bytes).unwrap();
        assert_eq!(recovered, value);
        assert_eq!(encoded_expression(&recovered), bytes);
    }
}

#[test]
fn all_call_target_variants_preserve_evidence_and_static_arguments() {
    let mut targets = BuiltinFunction::ALL
        .iter()
        .copied()
        .map(PackageReviewContractCallTarget::BuiltinFunction)
        .collect::<Vec<_>>();
    for predicate in [
        PackageReviewByteSequencePredicate::ValidUtf8,
        PackageReviewByteSequencePredicate::NoNul,
        PackageReviewByteSequencePredicate::AsciiOnly,
        PackageReviewByteSequencePredicate::NonEmpty,
    ] {
        targets.push(PackageReviewContractCallTarget::ByteSequencePredicate(
            predicate,
        ));
    }
    for operation in [
        PackageReviewCollectionViewOperation::SharedSlice,
        PackageReviewCollectionViewOperation::MutableSlice,
        PackageReviewCollectionViewOperation::TextView,
        PackageReviewCollectionViewOperation::Bytes,
    ] {
        targets.push(PackageReviewContractCallTarget::CollectionView(operation));
    }
    for target in targets {
        let value = call(target);
        assert_eq!(
            recovered_expression(&encoded_expression(&value)).unwrap(),
            value
        );
    }
}

#[test]
fn unknown_expression_tags_enum_values_and_builtin_ordinals_reject() {
    for bytes in [
        vec![22],
        vec![255],
        vec![0, 2],
        vec![7, 2],
        vec![6, 2],
        vec![6, 0, 18],
        vec![19, 2],
        vec![20, 3],
        vec![21, 3],
        vec![11, 0, 4],
        vec![11, 0, 1, 4],
        vec![11, 0, 3, 4],
        vec![11, 0, 2, 255, 255],
    ] {
        assert_eq!(recovered_expression(&bytes), Err(Error::InvalidTag));
    }
    assert_eq!(recovered_expression(&[11, 2]), Err(Error::InvalidTag));
    for bytes in [vec![10], vec![255], vec![7, 2]] {
        assert_eq!(recovered_static(&bytes), Err(Error::InvalidTag));
    }
}

#[test]
fn evidence_lane_kinds_and_incomplete_terms_reject() {
    let value = call(PackageReviewContractCallTarget::BuiltinFunction(
        BuiltinFunction::IntegerEmbed,
    ));
    let original = encoded_expression(&value);
    for owner in [
        b"Source::enter".as_slice(),
        b"Destination::enter".as_slice(),
    ] {
        let position = original
            .windows(owner.len())
            .position(|bytes| bytes == owner)
            .unwrap();
        let kind_position = position + owner.len();
        let mut changed = original.clone();
        changed[kind_position] = 2;
        assert_eq!(recovered_expression(&changed), Err(Error::InvalidTag));
        assert!(recovered_expression(&original[..kind_position + 1]).is_err());
    }
}

#[test]
fn expression_recovery_rejects_truncation_trailing_bytes_counts_and_depth() {
    let value = call(PackageReviewContractCallTarget::BuiltinFunction(
        BuiltinFunction::IntegerEmbed,
    ));
    let bytes = encoded_expression(&value);
    for length in 0..bytes.len() {
        assert!(recovered_expression(&bytes[..length]).is_err());
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(recovered_expression(&trailing), Err(Error::TrailingBytes));
    let mut impossible = vec![14];
    impossible.extend_from_slice(&u64::MAX.to_le_bytes());
    assert!(recovered_expression(&impossible).is_err());
    let mut deep = vec![];
    for _ in 0..256 {
        deep.extend_from_slice(&[7, 0]);
    }
    deep.push(3);
    assert_eq!(
        recovered_expression(&deep),
        Err(Error::NestingLimitExceeded)
    );
}
