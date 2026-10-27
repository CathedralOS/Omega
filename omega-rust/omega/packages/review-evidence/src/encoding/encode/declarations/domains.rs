use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewDomainAliasAtom, PackageReviewDomainEstablishmentKind,
    PackageReviewDomainEstablishmentRoute,
};

pub(crate) fn encode_domain_alias_atom(
    encoder: &mut Encoder,
    atom: &PackageReviewDomainAliasAtom,
) -> Result<(), PackageReviewEncodingError> {
    match atom {
        PackageReviewDomainAliasAtom::Declared(identity) => {
            encoder.tag("declared", 0);
            encoder.field("identity", |encoder| encode_nominal(encoder, identity))
        }
        PackageReviewDomainAliasAtom::Carry(permission) => {
            encoder.tag("carry", 1);
            encoder.field("permission", |encoder| {
                encode_carry_permission(encoder, *permission);
                Ok(())
            })?;
            Ok(())
        }
    }
}

pub(crate) fn encode_domain_establishment_route(
    encoder: &mut Encoder,
    route: &PackageReviewDomainEstablishmentRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("kind", |encoder| {
        match route.kind() {
            PackageReviewDomainEstablishmentKind::CheckedRequirement => {
                encoder.tag("checked_requirement", 0)
            }
            PackageReviewDomainEstablishmentKind::BoundaryRequirement => {
                encoder.tag("boundary_requirement", 1)
            }
            PackageReviewDomainEstablishmentKind::ExactMachine => encoder.tag("exact_machine", 2),
        };
        Ok(())
    })?;
    // Tags 0 and 1 retain their original two-identity layout. Tag 2 adds
    // exactly one callable identity, without a synthetic requirement owner.
    match route {
        PackageReviewDomainEstablishmentRoute::CheckedRequirement {
            trait_identity,
            requirement_identity,
        }
        | PackageReviewDomainEstablishmentRoute::BoundaryRequirement {
            trait_identity,
            requirement_identity,
        } => {
            encoder.field_by("trait_identity", trait_identity, encode_nominal)?;
            encoder.field_by("requirement_identity", requirement_identity, encode_nominal)
        }
        PackageReviewDomainEstablishmentRoute::ExactMachine { machine_identity } => encoder
            .field("machine_identity", |encoder| {
                encode_nominal(encoder, machine_identity)
            }),
    }
}

/// Shared closed permission encoding for alias atoms and contract facts.
pub(crate) fn encode_carry_permission(
    encoder: &mut Encoder,
    permission: language_semantics::CarryPermission,
) {
    match permission {
        language_semantics::CarryPermission::AcrossSuspend => encoder.tag("across_suspend", 0),
        language_semantics::CarryPermission::AnyCpu => encoder.tag("any_cpu", 1),
        language_semantics::CarryPermission::AnyThread => encoder.tag("any_thread", 2),
        language_semantics::CarryPermission::MovableAddress => encoder.tag("movable_address", 3),
    };
}
