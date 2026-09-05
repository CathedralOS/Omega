use super::super::{Error, identity::nominal, reader::Reader};
#[cfg(test)]
use super::super::{contracts::contract_fact, identity::type_identity, signatures::type_parameter};
use crate::record::*;

#[cfg(test)]
pub(in crate::encoding::recovery::policy) fn domain_shape(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDomainShape, Error> {
    Ok(PackageReviewDomainShape {
        identity: nominal(reader)?,
        type_parameters: reader.sequence(3, type_parameter)?,
        target_type: type_identity(reader)?,
        index_arguments: reader.sequence(8, type_identity)?,
        predicate_body: match reader.byte()? {
            0 => language_semantics::DomainPredicateBody::Bodyless,
            1 => language_semantics::DomainPredicateBody::Present,
            _ => return Err(Error::InvalidTag),
        },
        predicate_facts: reader.sequence(2, contract_fact)?,
        alias_expansion: reader.option(|reader| reader.sequence(2, alias_atom))?,
        classification: match reader.byte()? {
            0 => None,
            1 => Some(PackageReviewDomainClassification::ProgressProfile),
            _ => return Err(Error::InvalidTag),
        },
        semantic_roles: reader.sequence(1, |reader| {
            Ok(match reader.byte()? {
                0 => PackageReviewDomainSemanticRole::DenotationDimension,
                1 => PackageReviewDomainSemanticRole::ArithmeticPolicy,
                _ => return Err(Error::InvalidTag),
            })
        })?,
        establishment_routes: reader.sequence(83, |reader| {
            Ok(PackageReviewDomainEstablishmentRoute {
                kind: match reader.byte()? {
                    0 => PackageReviewDomainEstablishmentKind::CheckedRequirement,
                    1 => PackageReviewDomainEstablishmentKind::BoundaryRequirement,
                    _ => return Err(Error::InvalidTag),
                },
                trait_identity: nominal(reader)?,
                requirement_identity: nominal(reader)?,
            })
        })?,
    })
}

pub(in crate::encoding::recovery::policy) fn alias_atom(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDomainAliasAtom, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewDomainAliasAtom::Declared(nominal(reader)?),
        1 => PackageReviewDomainAliasAtom::Carry(match reader.byte()? {
            0 => language_semantics::CarryPermission::AcrossSuspend,
            1 => language_semantics::CarryPermission::AnyCpu,
            2 => language_semantics::CarryPermission::AnyThread,
            3 => language_semantics::CarryPermission::MovableAddress,
            _ => return Err(Error::InvalidTag),
        }),
        _ => return Err(Error::InvalidTag),
    })
}
