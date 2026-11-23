// Bounded recovery for domain alias declarations and establishment routes.
// `establishment_route` requires at least 42 bytes per route (tag + nominal owner tag/digest + empty path length); requirement variants additionally carry a second nominal.

use super::super::{Error, identity::nominal, reader::Reader};
use crate::record::{PackageReviewDomainEstablishmentRoute, PackageReviewNominalIdentity};

pub(in crate::encoding::recovery) fn alias_atom(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewNominalIdentity, Error> {
    Ok(super::super::tag!(reader => {
        0 => nominal(reader)?,
        1 => return Err(Error::RetiredVocabulary),
    }))
}

pub(in crate::encoding::recovery) fn establishment_route(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDomainEstablishmentRoute, Error> {
    Ok(super::super::tag!(reader => {
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
