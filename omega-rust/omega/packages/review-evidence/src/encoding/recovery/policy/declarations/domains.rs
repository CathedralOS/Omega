use super::super::{Error, identity::nominal, reader::Reader};
use crate::record::{PackageReviewDomainAliasAtom, PackageReviewDomainEstablishmentRoute};

pub(in crate::encoding::recovery) fn alias_atom(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDomainAliasAtom, Error> {
    Ok(super::super::tag!(reader => {
        0 => PackageReviewDomainAliasAtom::Declared(nominal(reader)?),
        1 => PackageReviewDomainAliasAtom::Carry(carry_permission(reader)?),
    }))
}

/// Minimum route size is tag + nominal owner tag/digest + empty path length:
/// 42 bytes. Requirement variants additionally carry a second nominal.
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

pub(in crate::encoding::recovery) fn carry_permission(
    reader: &mut Reader<'_>,
) -> Result<language_semantics::CarryPermission, Error> {
    Ok(super::super::tag!(reader => {
        0 => language_semantics::CarryPermission::AcrossSuspend,
        1 => language_semantics::CarryPermission::AnyCpu,
        2 => language_semantics::CarryPermission::AnyThread,
        3 => language_semantics::CarryPermission::MovableAddress,
    }))
}
