use super::super::{
    Error,
    contracts::{contract_fact, evidence_interface, proposition_binder},
    identity::{nominal, type_identity},
    reader::Reader,
};
use crate::record::{
    PackageReviewConstMeaning, PackageReviewConstShape, PackageReviewPropositionShape,
    PackageReviewPublicPropositionBody,
};

pub(in crate::encoding::recovery) fn proposition_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPropositionShape, Error> {
    Ok(PackageReviewPropositionShape {
        identity: nominal(reader)?,
        binders: reader.sequence(3, proposition_binder)?,
        parameter_types: reader.sequence(8, type_identity)?,
        body: super::super::tag!(reader => {
            0 => PackageReviewPublicPropositionBody::Primitive,
            1 => PackageReviewPublicPropositionBody::Witness(evidence_interface(reader)?),
            2 => PackageReviewPublicPropositionBody::Transparent(contract_fact(reader)?),
        }),
    })
}

pub(in crate::encoding::recovery) fn const_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewConstShape, Error> {
    Ok(PackageReviewConstShape {
        identity: nominal(reader)?,
        declared_type: type_identity(reader)?,
        meaning: super::super::tag!(reader => {
            0 => PackageReviewConstMeaning::Evaluated(reader.string()?),
            1 => PackageReviewConstMeaning::TargetDependent(super::super::expressions::expression(reader)?),
        }),
    })
}

pub(in crate::encoding::recovery) fn operator_spelling(
    reader: &mut Reader<'_>,
) -> Result<language_core::OperatorSpelling, Error> {
    use language_core::OperatorSpelling;
    Ok(super::super::tag!(reader => {
        0 => OperatorSpelling::Add,
        1 => OperatorSpelling::Subtract,
        2 => OperatorSpelling::Multiply,
        3 => OperatorSpelling::Divide,
        4 => OperatorSpelling::Modulo,
        5 => OperatorSpelling::Equal,
        6 => OperatorSpelling::NotEqual,
        7 => OperatorSpelling::Less,
        8 => OperatorSpelling::LessEqual,
        9 => OperatorSpelling::Greater,
        10 => OperatorSpelling::GreaterEqual,
        11 => OperatorSpelling::Index,
        12 => OperatorSpelling::Range,
    }))
}

#[cfg(test)]
mod constant_meaning_tests {
    use super::super::super::PackagePolicyRecoveryLimits;
    use super::*;

    fn prefix() -> Vec<u8> {
        let mut bytes = vec![0]; // Exact package owner of the public declaration.
        bytes.extend_from_slice(&[1; 32]);
        for text in ["RAW_SIZE", "named(name(u64))"] {
            bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        bytes
    }

    #[test]
    fn deferred_constant_meaning_never_recovers_as_a_folded_value() {
        let canonical = "named(name(u8))";
        let mut bytes = prefix();
        bytes.extend_from_slice(&[1, 27, 0]); // Dependency, size query.
        bytes.extend_from_slice(&(canonical.len() as u64).to_le_bytes());
        bytes.extend_from_slice(canonical.as_bytes());
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        let shape = const_shape(&mut reader).unwrap();
        assert!(matches!(
            shape.meaning,
            PackageReviewConstMeaning::TargetDependent(
                crate::record::PackageReviewContractExpression::LayoutQuery {
                    kind: crate::record::PackageReviewLayoutQueryKind::Size,
                    ..
                }
            )
        ));
        reader.finish().unwrap();
        for suffix in [
            &[2][..],
            &[255][..],
            &[1][..],
            &[1, 27, 2][..],
            &[1, 5, 2][..],
        ] {
            let mut bytes = prefix();
            bytes.extend_from_slice(suffix);
            let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
            assert!(const_shape(&mut reader).is_err());
        }
    }
}
