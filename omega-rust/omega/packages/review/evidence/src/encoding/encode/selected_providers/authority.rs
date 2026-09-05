use super::{Encoder, PackageReviewEncodingError, encode_nominal};
use crate::encoding::encode::values::effects::encode_synchronous_invocation;
use crate::record::PackagePolicyServiceAuthority;
use omega_effects::provider_plan::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};

pub(super) fn authority(
    encoder: &mut Encoder,
    authority: &PackagePolicyServiceAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence(&authority.service_reach, encode_nominal)?;
    encoder.sequence(
        &authority.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.sequence(&authority.progress_premises, |encoder, premise| {
        encode_nominal(encoder, &premise.profile)?;
        match premise.subject {
            ServiceProgressSubject::ProviderReceiver => encoder.byte(0),
            ServiceProgressSubject::Parameter(position) => {
                encoder.byte(1);
                encoder.usize(position)?;
            }
        }
        encoder.sequence(&premise.subject_projections, encode_nominal)?;
        encoder.sequence(&premise.establishment_routes, |encoder, route| {
            encoder.byte(match route.kind {
                ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
            });
            encode_nominal(encoder, &route.requirement_owner)?;
            encode_nominal(encoder, &route.requirement)
        })
    })
}
