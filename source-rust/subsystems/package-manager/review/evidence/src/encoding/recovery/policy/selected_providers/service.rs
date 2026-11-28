// Bounded recovery for selected-provider service methods, entry/result claims, and carry policies.
// Interned route identities in `method` rebuild from the same canonical texts they encoded; mint order is record-local and never part of the persisted bytes.

use super::{Error, Reader, nominal};
use crate::encoding::recovery::policy::calling_application;
use crate::record::PackagePolicyServiceMethod;
use language_core::CarryAddress;
use language_core::CarryCpu;
use language_core::CarryHostThread;
use language_core::CarryPolicy;
use language_core::CarrySuspension;
use language_core::DomainPredicatePresence;
use service_surface::{
    ServiceDomainMembership, ServiceMembershipProjection, ServiceSynchronousInvocation,
};
use service_surface::{
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim,
};

pub(in crate::encoding::recovery) fn method(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyServiceMethod, Error> {
    let mut identities = service_surface::identities::ProviderIdentityTable::default();
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
                predicate_presence: super::super::tag!(reader => {
                    0 => DomainPredicatePresence::Absent,
                    1 => DomainPredicatePresence::Present,
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
        domain_memberships: reader.sequence(16, membership)?,
        result_claims: reader.sequence(12, |reader| {
            Ok(ServiceResultClaim {
                domain: reader.string()?,
                effective_carry: carry(reader)?,
            })
        })?,
        service_reach: reader.sequence(8, Reader::string)?,
        synchronous_invocations: reader.sequence(5, |reader| {
            Ok(super::super::tag!(reader => {
                1 => ServiceSynchronousInvocation::Parameter(reader.u32()?),
                2 => ServiceSynchronousInvocation::Service(reader.string()?),
                3 => super::super::behavior::projected_parameter(reader)?,
            }))
        })?,
        may_suspend: reader.boolean()?,
        may_block: reader.boolean()?,
        admitted_same_stack_bytes: reader.u64()?,
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
        calling: reader.sequence(1, calling_application::application)?,
    })
}

fn membership(reader: &mut Reader<'_>) -> Result<ServiceDomainMembership, Error> {
    use service_surface::ServiceMembershipSubject;
    let mut subject = ServiceMembershipSubject::NormalResult;
    let (count, mut path) = reader.sequence_buffer(5)?;
    for position in 0..count {
        path.push(super::super::tag!(reader => {
            1 => ServiceMembershipProjection::Field(reader.string()?),
            2 => ServiceMembershipProjection::Case(reader.string()?),
            3 => ServiceMembershipProjection::FixedIndex(reader.u64()?),
            4 => {
                if position != 0 { return Err(Error::InvalidValue); }
                subject = ServiceMembershipSubject::Input(reader.u32()?);
                continue;
            },
        }));
    }
    Ok(ServiceDomainMembership {
        subject,
        path,
        domain: reader.string()?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::encode::{encode_membership, encoder::Encoder};
    use crate::encoding::recovery::policy::PackagePolicyRecoveryLimits;
    use service_surface::ServiceMembershipSubject;

    #[test]
    fn a_reviewed_membership_preserves_result_bytes_and_the_exact_input_ordinal() {
        let result = ServiceDomainMembership {
            subject: ServiceMembershipSubject::NormalResult,
            path: vec![ServiceMembershipProjection::FixedIndex(3)],
            domain: "Tickets::Issued".into(),
        };
        let mut projected = vec![3];
        projected.extend(3u64.to_le_bytes());
        let mut domain_bytes = (result.domain.len() as u64).to_le_bytes().to_vec();
        domain_bytes.extend(result.domain.as_bytes());
        for subject in [
            ServiceMembershipSubject::NormalResult,
            ServiceMembershipSubject::Input(1),
        ] {
            let row = ServiceDomainMembership {
                subject,
                ..result.clone()
            };
            let mut encoder = Encoder::bounded(128);
            encode_membership(&mut encoder, &row).unwrap();
            let encoded = encoder.finish().unwrap();
            let mut expected = match subject {
                ServiceMembershipSubject::NormalResult => 1u64.to_le_bytes().to_vec(),
                ServiceMembershipSubject::Input(ordinal) => {
                    let mut prefix = 2u64.to_le_bytes().to_vec();
                    prefix.push(4);
                    prefix.extend(ordinal.to_le_bytes());
                    prefix
                }
            };
            expected.extend(&projected);
            expected.extend(&domain_bytes);
            assert_eq!(encoded, expected);
            let mut reader = Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap();
            assert_eq!(membership(&mut reader).unwrap(), row);
            reader.finish().unwrap();
        }

        let mut late_subject = 2u64.to_le_bytes().to_vec();
        late_subject.extend(projected);
        late_subject.push(4);
        late_subject.extend(1u32.to_le_bytes());
        late_subject.extend(domain_bytes);
        let mut reader =
            Reader::new(&late_subject, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(matches!(membership(&mut reader), Err(Error::InvalidValue)));
    }
}
