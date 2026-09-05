use super::{Encoder, PackageReviewEncodingError, encode_nominal};
use crate::encoding::encode::{
    calling::encode_application, values::providers::encode_carry_policy,
};
use crate::record::PackagePolicyServiceMethod;
use omega_effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceProgressEstablishmentRouteKind, ServiceProgressSubject,
};

pub(in crate::encoding::encode) fn method(
    encoder: &mut Encoder,
    method: &PackagePolicyServiceMethod,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&method.name)?;
    encode_nominal(encoder, &method.requirement_owner)?;
    encode_nominal(encoder, &method.requirement)?;
    super::signature::signature(encoder, &method.signature)?;
    super::authority::authority(encoder, &method.authority)?;
    encoder.usize(method.parameter_count)?;
    encoder.sequence(&method.parameter_type_identities, |encoder, value| {
        encoder.string(value)
    })?;
    encoder.sequence(&method.entry_claims, |encoder, claim| {
        encoder.usize(claim.parameter_index)?;
        encoder.string(&claim.carrier_identity)?;
        encoder.string(&claim.domain)?;
        encoder.byte(match claim.predicate_body {
            psi_language_semantics::DomainPredicateBody::Bodyless => 0,
            psi_language_semantics::DomainPredicateBody::Present => 1,
        });
        encode_carry_policy(encoder, claim.effective_carry);
        encoder.byte(match claim.authority_flow {
            ServiceEntryAuthorityFlow::Accepts => 0,
        });
        Ok(())
    })?;
    encoder.boolean(method.has_result);
    encoder.option(method.result_type_identity.as_ref(), |encoder, value| {
        encoder.string(value)
    })?;
    encoder.sequence(&method.result_claims, |encoder, claim| {
        encoder.string(&claim.domain)?;
        encode_carry_policy(encoder, claim.effective_carry);
        Ok(())
    })?;
    encoder.sequence(&method.service_reach, |encoder, value| {
        encoder.string(value)
    })?;
    encoder.sequence(&method.synchronous_invocations, |encoder, value| {
        encoder.string(value)
    })?;
    encoder.boolean(method.may_suspend);
    encoder.boolean(method.may_block);
    encoder.boolean(method.terminates_guarantee);
    encoder.sequence(&method.termination_premises, |encoder, premise| {
        encoder.string(&premise.profile)?;
        match premise.subject {
            ServiceProgressSubject::ProviderReceiver => encoder.byte(0),
            ServiceProgressSubject::Parameter(position) => {
                encoder.byte(1);
                encoder.usize(position)?;
            }
        }
        encoder.sequence(&premise.subject_projections, |encoder, value| {
            encoder.string(value)
        })?;
        encoder.sequence(&premise.establishment_routes, |encoder, route| {
            encoder.byte(match route.kind {
                ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
            });
            encoder.string(&route.requirement_identity)
        })
    })?;
    encoder.option(method.calling.as_ref(), encode_application)
}
