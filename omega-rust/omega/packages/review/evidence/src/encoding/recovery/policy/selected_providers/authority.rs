use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::behavior::synchronous_invocation;
use crate::record::{
    PackagePolicyServiceAuthority, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute,
};
use omega_effects::provider_plan::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};

pub(super) fn authority(reader: &mut Reader<'_>) -> Result<PackagePolicyServiceAuthority, Error> {
    Ok(PackagePolicyServiceAuthority {
        service_reach: reader.sequence(41, nominal)?,
        synchronous_invocations: reader.sequence(5, synchronous_invocation)?,
        progress_premises: reader.sequence(58, |reader| {
            Ok(PackagePolicyServiceProgressPremise {
                profile: nominal(reader)?,
                subject: match reader.byte()? {
                    0 => ServiceProgressSubject::ProviderReceiver,
                    1 => ServiceProgressSubject::Parameter(reader.usize()?),
                    _ => return Err(Error::InvalidTag),
                },
                subject_projections: reader.sequence(41, nominal)?,
                establishment_routes: reader.sequence(83, |reader| {
                    Ok(PackagePolicyServiceProgressRoute {
                        kind: match reader.byte()? {
                            0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                            1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                            _ => return Err(Error::InvalidTag),
                        },
                        requirement_owner: nominal(reader)?,
                        requirement: nominal(reader)?,
                    })
                })?,
            })
        })?,
    })
}
