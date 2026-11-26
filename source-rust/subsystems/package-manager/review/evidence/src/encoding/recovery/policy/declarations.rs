// Full public declaration vocabulary recovery under the enclosing policy reader.

use super::Error;
use super::contracts::{contract_fact, evidence_interface, proposition_binder};
use super::identity::{nominal, type_identity};
use super::reader::Reader;
use crate::record::{
    PackageReviewConstMeaning, PackageReviewConstShape, PackageReviewDataField,
    PackageReviewDataMember, PackageReviewDomainEstablishmentRoute, PackageReviewNominalIdentity,
    PackageReviewPropositionShape, PackageReviewPublicPropositionBody,
    PackageReviewTraitCompositionKind, PackageReviewTraitParent,
};

// Bounded recovery for data members, variants, and fields in public declarations.

pub(in crate::encoding::recovery) fn data_member(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataMember, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewDataMember::Field(data_field(reader)?),
        1 => PackageReviewDataMember::Variant {
            identity: reader.option(|reader| reader.u64())?,
            name: reader.string()?,
            payload: reader.sequence(18, data_field)?,
            retired_payload_identities: reader.sequence(8, |reader| reader.u64())?,
        },
    }))
}

pub(in crate::encoding::recovery) fn data_field(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDataField, Error> {
    Ok(PackageReviewDataField {
        identity: reader.option(|reader| reader.u64())?,
        name: reader.string()?,
        relevance: super::tag!(reader => {
            0 => language_core::BindingRelevance::Relevant,
            1 => language_core::BindingRelevance::Erased,
        }),
        type_identity: type_identity(reader)?,
    })
}

// Bounded recovery for domain alias declarations and establishment routes.
// `establishment_route` requires at least 42 bytes per route (tag + nominal owner tag/digest + empty path length); requirement variants additionally carry a second nominal.

pub(in crate::encoding::recovery) fn alias_atom(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewNominalIdentity, Error> {
    Ok(super::tag!(reader => {
        0 => nominal(reader)?,
        1 => return Err(Error::RetiredVocabulary),
    }))
}

pub(in crate::encoding::recovery) fn establishment_route(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDomainEstablishmentRoute, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewDomainEstablishmentRoute::CheckedRequirement {
            trait_identity: nominal(reader)?,
            requirement_identity: nominal(reader)?,
        },
        1 => PackageReviewDomainEstablishmentRoute::BoundaryRequirement {
            trait_identity: nominal(reader)?,
            requirement_identity: nominal(reader)?,
        },
        2 => PackageReviewDomainEstablishmentRoute::ExactMachine {
            machine_identity: nominal(reader)?,
        },
    }))
}

// Bounded recovery for trait parent composition bounds and arguments.

pub(in crate::encoding::recovery) fn trait_parent(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewTraitParent, Error> {
    Ok(PackageReviewTraitParent {
        kind: super::tag!(reader => {
            0 => PackageReviewTraitCompositionKind::Policy,
            1 => PackageReviewTraitCompositionKind::ServiceReach,
        }),
        identity: nominal(reader)?,
        lifetime_arguments: reader.sequence(4, |reader| reader.u32())?,
        arguments: reader.sequence(8, type_identity)?,
    })
}

// Bounded recovery for proposition shapes, constant shapes, and operator spellings.

pub(in crate::encoding::recovery) fn proposition_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewPropositionShape, Error> {
    Ok(PackageReviewPropositionShape {
        identity: nominal(reader)?,
        binders: reader.sequence(3, proposition_binder)?,
        parameter_types: reader.sequence(8, type_identity)?,
        body: super::tag!(reader => {
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
        meaning: super::tag!(reader => {
            0 => PackageReviewConstMeaning::Evaluated(reader.string()?),
            1 => PackageReviewConstMeaning::TargetDependent(super::expressions::expression(reader)?),
        }),
    })
}

pub(in crate::encoding::recovery) fn operator_spelling(
    reader: &mut Reader<'_>,
) -> Result<language_core::OperatorSpelling, Error> {
    use language_core::OperatorSpelling;
    Ok(super::tag!(reader => {
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
        let mut bytes = vec![0];
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
        bytes.extend_from_slice(&[1, 27, 0]);
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
