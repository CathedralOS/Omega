// Canonical encoding for domain alias declarations and establishment routes.

use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewNominalIdentity,
};

pub(crate) fn encode_domain_alias_atom(
    encoder: &mut Encoder,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.tag("declared", 0);
    encoder.field("identity", |encoder| encode_nominal(encoder, identity))
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
