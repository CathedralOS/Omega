use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::calling_application;
use crate::record::PackagePolicyServiceMethod;
use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim,
};
use psi_language_semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, DomainPredicateBody,
};

pub(in crate::encoding::recovery::policy) fn method(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyServiceMethod, Error> {
    Ok(PackagePolicyServiceMethod {
        name: reader.string()?,
        requirement_owner: nominal(reader)?,
        requirement: nominal(reader)?,
        signature: super::signature::signature(reader)?,
        authority: super::authority::authority(reader)?,
        parameter_count: reader.usize()?,
        parameter_type_identities: reader.sequence(8, Reader::string)?,
        entry_claims: reader.sequence(30, |reader| {
            Ok(ServiceEntryClaim {
                parameter_index: reader.usize()?,
                carrier_identity: reader.string()?,
                domain: reader.string()?,
                predicate_body: match reader.byte()? {
                    0 => DomainPredicateBody::Bodyless,
                    1 => DomainPredicateBody::Present,
                    _ => return Err(Error::InvalidTag),
                },
                effective_carry: carry(reader)?,
                authority_flow: match reader.byte()? {
                    0 => ServiceEntryAuthorityFlow::Accepts,
                    _ => return Err(Error::InvalidTag),
                },
            })
        })?,
        has_result: reader.boolean()?,
        result_type_identity: reader.option(Reader::string)?,
        result_claims: reader.sequence(12, |reader| {
            Ok(ServiceResultClaim {
                domain: reader.string()?,
                effective_carry: carry(reader)?,
            })
        })?,
        service_reach: reader.sequence(8, Reader::string)?,
        synchronous_invocations: reader.sequence(8, Reader::string)?,
        may_suspend: reader.boolean()?,
        may_block: reader.boolean()?,
        terminates_guarantee: reader.boolean()?,
        termination_premises: reader.sequence(25, |reader| {
            Ok(ServiceProgressPremise {
                profile: reader.string()?,
                subject: match reader.byte()? {
                    0 => ServiceProgressSubject::ProviderReceiver,
                    1 => ServiceProgressSubject::Parameter(reader.usize()?),
                    _ => return Err(Error::InvalidTag),
                },
                subject_projections: reader.sequence(8, Reader::string)?,
                establishment_routes: reader.sequence(9, |reader| {
                    Ok(ServiceProgressEstablishmentRoute {
                        kind: match reader.byte()? {
                            0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                            1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                            _ => return Err(Error::InvalidTag),
                        },
                        requirement_identity: reader.string()?,
                    })
                })?,
            })
        })?,
        calling: reader.option(calling_application::application)?,
    })
}

fn carry(reader: &mut Reader<'_>) -> Result<CarryPolicy, Error> {
    Ok(CarryPolicy {
        suspension: match reader.byte()? {
            0 => CarrySuspension::Forbidden,
            1 => CarrySuspension::Allowed,
            _ => return Err(Error::InvalidTag),
        },
        cpu: match reader.byte()? {
            0 => CarryCpu::Origin,
            1 => CarryCpu::Any,
            _ => return Err(Error::InvalidTag),
        },
        host_thread: match reader.byte()? {
            0 => CarryHostThread::Origin,
            1 => CarryHostThread::Any,
            _ => return Err(Error::InvalidTag),
        },
        address: match reader.byte()? {
            0 => CarryAddress::Stable,
            1 => CarryAddress::Movable,
            _ => return Err(Error::InvalidTag),
        },
    })
}
