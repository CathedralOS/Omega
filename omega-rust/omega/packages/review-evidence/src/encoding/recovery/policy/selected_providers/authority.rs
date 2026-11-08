use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::behavior::synchronous_invocation;
use crate::record::{
    PackagePolicyServiceAuthority, PackagePolicyServiceProgressPremise,
    PackagePolicyServiceProgressRoute,
};
use effects::provider_plan::{ServiceProgressEstablishmentRouteKind, ServiceProgressSubject};

pub(in crate::encoding::recovery) fn authority(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyServiceAuthority, Error> {
    Ok(PackagePolicyServiceAuthority {
        service_reach: reader.sequence(41, nominal)?,
        synchronous_invocations: reader.sequence(5, synchronous_invocation)?,
        progress_premises: reader.sequence(58, |reader| {
            Ok(PackagePolicyServiceProgressPremise {
                profile: nominal(reader)?,
                subject: super::super::tag!(reader => {
                    1 => ServiceProgressSubject::Parameter(reader.usize()?),
                }),
                subject_projections: reader.sequence(41, nominal)?,
                establishment_routes: reader.sequence(83, |reader| {
                    Ok(PackagePolicyServiceProgressRoute {
                        kind: super::super::tag!(reader => {
                            0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                            1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                        }),
                        requirement_owner: nominal(reader)?,
                        requirement: nominal(reader)?,
                    })
                })?,
            })
        })?,
    })
}
