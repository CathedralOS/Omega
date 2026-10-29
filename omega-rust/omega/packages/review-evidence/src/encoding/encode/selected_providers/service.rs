use super::{Encoder, PackageReviewEncodingError, encode_nominal};
use crate::encoding::encode::{
    calling::encode_application, values::providers::encode_carry_policy,
};
use crate::record::PackagePolicyServiceMethod;
use effects::provider_plan::{
    ServiceEntryAuthorityFlow, ServiceProgressEstablishmentRouteKind, ServiceProgressSubject,
};

pub(in crate::encoding::encode) fn method(
    encoder: &mut Encoder,
    method: &PackagePolicyServiceMethod,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("name", |encoder| encoder.string(&method.name))?;
    encoder.field_by(
        "requirement_owner",
        &method.requirement_owner,
        encode_nominal,
    )?;
    encoder.field_by("requirement", &method.requirement, encode_nominal)?;
    encoder.field_by("signature", &method.signature, super::signature::signature)?;
    encoder.field_by("authority", &method.authority, super::authority::authority)?;
    encoder.usize_field("parameter_count", method.parameter_count)?;
    encoder.field("parameter_type_identities", |encoder| {
        encoder.sequence(&method.parameter_type_identities, |encoder, value| {
            encoder.field("value", |encoder| encoder.string(value))
        })
    })?;
    encoder.field("entry_claims", |encoder| {
        encoder.sequence(&method.entry_claims, |encoder, claim| {
            encoder.usize_field("parameter_index", claim.parameter_index)?;
            encoder.string_field(
                "carrier_identity",
                method.identities.text(claim.carrier_identity),
            )?;
            encoder.field("domain", |encoder| encoder.string(&claim.domain))?;
            encoder.field("predicate_presence", |encoder| {
                match claim.predicate_presence {
                    language_semantics::DomainPredicatePresence::Absent => {
                        encoder.tag("bodyless", 0)
                    }
                    language_semantics::DomainPredicatePresence::Present => {
                        encoder.tag("present", 1)
                    }
                };
                Ok(())
            })?;
            encoder.field_by(
                "effective_carry",
                claim.effective_carry,
                encode_carry_policy,
            )?;
            encoder.field("authority_flow", |encoder| {
                match claim.authority_flow {
                    ServiceEntryAuthorityFlow::Accepts => encoder.tag("accepts", 0),
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("completion", |encoder| {
        match method.completion {
            semantic_vocabulary::BoundaryCompletion::NormalReturn => {
                encoder.tag("normal_return", 0)
            }
            semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            ) => encoder.tag("process_exit", 1),
        };
        Ok(())
    })?;
    encoder.boolean_field("has_result", method.has_result)?;
    encoder.field("result_type_identity", |encoder| {
        encoder.option(method.result_type_identity.as_ref(), |encoder, value| {
            encoder.field("value", |encoder| encoder.string(value))
        })
    })?;
    encoder.field("result_claims", |encoder| {
        encoder.sequence(&method.result_claims, |encoder, claim| {
            encoder.field("domain", |encoder| encoder.string(&claim.domain))?;
            encoder.field_by(
                "effective_carry",
                claim.effective_carry,
                encode_carry_policy,
            )?;
            Ok(())
        })
    })?;
    encoder.field("service_reach", |encoder| {
        encoder.sequence(&method.service_reach, |encoder, value| {
            encoder.field("value", |encoder| encoder.string(value))
        })
    })?;
    encoder.field("synchronous_invocations", |encoder| {
        encoder.sequence(&method.synchronous_invocations, |encoder, value| {
            encoder.field("value", |encoder| encoder.string(value))
        })
    })?;
    encoder.boolean_field("may_suspend", method.may_suspend)?;
    encoder.boolean_field("may_block", method.may_block)?;
    encoder.boolean_field("terminates_guarantee", method.terminates_guarantee)?;
    encoder.field("termination_premises", |encoder| {
        encoder.sequence(&method.termination_premises, |encoder, premise| {
            encoder.field("profile", |encoder| encoder.string(&premise.profile))?;
            encoder.field("subject", |encoder| {
                match premise.subject {
                    ServiceProgressSubject::Parameter(position) => {
                        encoder.tag("parameter", 1);
                        encoder.field("position", |encoder| encoder.usize(position))?;
                    }
                };
                Ok(())
            })?;
            encoder.field("subject_projections", |encoder| {
                encoder.sequence(&premise.subject_projections, |encoder, value| {
                    encoder.field("value", |encoder| encoder.string(value))
                })
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
                    encoder.string_field(
                        "requirement_identity",
                        method.identities.text(route.requirement_identity),
                    )
                })
            })
        })
    })?;
    encoder.option_field("calling", method.calling.as_ref(), encode_application)
}
