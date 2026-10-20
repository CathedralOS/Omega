use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::calling_application;
use crate::record::PackagePolicyServiceMethod;
use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim,
};
use language_semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, DomainPredicateBody,
};

pub(in crate::encoding::recovery) fn method(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyServiceMethod, Error> {
    // Interned route identities rebuild from the same canonical texts they
    // encoded; the mint order is record-local and never part of the bytes.
    let mut identities = effects::provider_plan::ProviderIdentityTable::default();
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
                carrier_identity: identities.intern(&reader.string()?),
                domain: reader.string()?,
                predicate_body: super::super::tag!(reader => {
                    0 => DomainPredicateBody::Bodyless,
                    1 => DomainPredicateBody::Present,
                }),
                effective_carry: carry(reader)?,
                authority_flow: super::super::tag!(reader => {
                    0 => ServiceEntryAuthorityFlow::Accepts,
                }),
            })
        })?,
        completion: super::super::tag!(reader => {
            0 => semantic_vocabulary::BoundaryCompletion::NormalReturn,
            1 => semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            ),
        }),
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
                subject: super::super::tag!(reader => {
                    1 => ServiceProgressSubject::Parameter(reader.usize()?),
                }),
                subject_projections: reader.sequence(8, Reader::string)?,
                establishment_routes: reader.sequence(9, |reader| {
                    Ok(ServiceProgressEstablishmentRoute {
                        kind: super::super::tag!(reader => {
                            0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                            1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                        }),
                        requirement_identity: identities.intern(&reader.string()?),
                    })
                })?,
            })
        })?,
        identities,
        calling: reader.option(calling_application::application)?,
    })
}

pub(in crate::encoding::recovery) fn carry(reader: &mut Reader<'_>) -> Result<CarryPolicy, Error> {
    Ok(CarryPolicy {
        suspension: super::super::tag!(reader => {
            0 => CarrySuspension::Forbidden,
            1 => CarrySuspension::Allowed,
        }),
        cpu: super::super::tag!(reader => {
            0 => CarryCpu::Origin,
            1 => CarryCpu::Any,
        }),
        host_thread: super::super::tag!(reader => {
            0 => CarryHostThread::Origin,
            1 => CarryHostThread::Any,
        }),
        address: super::super::tag!(reader => {
            0 => CarryAddress::Stable,
            1 => CarryAddress::Movable,
        }),
    })
}
