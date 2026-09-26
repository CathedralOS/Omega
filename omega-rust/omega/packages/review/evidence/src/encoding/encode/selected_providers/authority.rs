use super::{Encoder, PackageReviewEncodingError, encode_nominal};
use crate::encoding::encode::values::effects::encode_synchronous_invocation;
use crate::record::PackagePolicyServiceAuthority;
use effects::provider_plan::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};

pub(super) fn authority(
    encoder: &mut Encoder,
    authority: &PackagePolicyServiceAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("service_reach", |encoder| {
        encoder.sequence(&authority.service_reach, encode_nominal)
    })?;
    encoder.field("synchronous_invocations", |encoder| {
        encoder.sequence(
            &authority.synchronous_invocations,
            encode_synchronous_invocation,
        )
    })?;
    encoder.field("progress_premises", |encoder| {
        encoder.sequence(&authority.progress_premises, |encoder, premise| {
            encoder.field("profile", |encoder| {
                encode_nominal(encoder, &premise.profile)
            })?;
            encoder.field("subject", |encoder| {
                match premise.subject {
                    ServiceProgressSubject::ProviderReceiver => encoder.tag("provider_receiver", 0),
                    ServiceProgressSubject::Parameter(position) => {
                        encoder.tag("parameter", 1);
                        encoder.field("position", |encoder| encoder.usize(position))?;
                    }
                };
                Ok(())
            })?;
            encoder.field("subject_projections", |encoder| {
                encoder.sequence(&premise.subject_projections, encode_nominal)
            })?;
            encoder.field("establishment_routes", |encoder| {
                encoder.sequence(&premise.establishment_routes, |encoder, route| {
                    encoder.field("kind", |encoder| {
                        match route.kind {
                            ServiceProgressEstablishmentRouteKind::CheckedRequirement => {
                                encoder.tag("checked_requirement", 0)
                            }
                            ServiceProgressEstablishmentRouteKind::BoundaryRequirement => {
                                encoder.tag("boundary_requirement", 1)
                            }
                        };
                        Ok(())
                    })?;
                    encoder.field("requirement_owner", |encoder| {
                        encode_nominal(encoder, &route.requirement_owner)
                    })?;
                    encoder.field("requirement", |encoder| {
                        encode_nominal(encoder, &route.requirement)
                    })
                })
            })
        })
    })
}
