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
    encoder.field("name", |encoder| encoder.string(&method.name))?;
    encoder.field("requirement_owner", |encoder| {
        encode_nominal(encoder, &method.requirement_owner)
    })?;
    encoder.field("requirement", |encoder| {
        encode_nominal(encoder, &method.requirement)
    })?;
    encoder.field("signature", |encoder| {
        super::signature::signature(encoder, &method.signature)
    })?;
    encoder.field("authority", |encoder| {
        super::authority::authority(encoder, &method.authority)
    })?;
    encoder.field("parameter_count", |encoder| {
        encoder.usize(method.parameter_count)
    })?;
    encoder.field("parameter_type_identities", |encoder| {
        encoder.sequence(&method.parameter_type_identities, |encoder, value| {
            encoder.field("value", |encoder| encoder.string(value))
        })
    })?;
    encoder.field("entry_claims", |encoder| {
        encoder.sequence(&method.entry_claims, |encoder, claim| {
            encoder.field("parameter_index", |encoder| {
                encoder.usize(claim.parameter_index)
            })?;
            encoder.field("carrier_identity", |encoder| {
                encoder.string(&claim.carrier_identity)
            })?;
            encoder.field("domain", |encoder| encoder.string(&claim.domain))?;
            encoder.field("predicate_body", |encoder| {
                match claim.predicate_body {
                    psi_language_semantics::DomainPredicateBody::Bodyless => {
                        encoder.tag("bodyless", 0)
                    }
                    psi_language_semantics::DomainPredicateBody::Present => {
                        encoder.tag("present", 1)
                    }
                };
                Ok(())
            })?;
            encoder.field("effective_carry", |encoder| {
                encode_carry_policy(encoder, claim.effective_carry);
                Ok(())
            })?;
            encoder.field("authority_flow", |encoder| {
                match claim.authority_flow {
                    ServiceEntryAuthorityFlow::Accepts => encoder.tag("accepts", 0),
                };
                Ok(())
            })?;
            Ok(())
        })
    })?;
    encoder.field("has_result", |encoder| {
        encoder.boolean(method.has_result);
        Ok(())
    })?;
    encoder.field("result_type_identity", |encoder| {
        encoder.option(method.result_type_identity.as_ref(), |encoder, value| {
            encoder.field("value", |encoder| encoder.string(value))
        })
    })?;
    encoder.field("result_claims", |encoder| {
        encoder.sequence(&method.result_claims, |encoder, claim| {
            encoder.field("domain", |encoder| encoder.string(&claim.domain))?;
            encoder.field("effective_carry", |encoder| {
                encode_carry_policy(encoder, claim.effective_carry);
                Ok(())
            })?;
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
    encoder.field("may_suspend", |encoder| {
        encoder.boolean(method.may_suspend);
        Ok(())
    })?;
    encoder.field("may_block", |encoder| {
        encoder.boolean(method.may_block);
        Ok(())
    })?;
    encoder.field("terminates_guarantee", |encoder| {
        encoder.boolean(method.terminates_guarantee);
        Ok(())
    })?;
    encoder.field("termination_premises", |encoder| {
        encoder.sequence(&method.termination_premises, |encoder, premise| {
            encoder.field("profile", |encoder| encoder.string(&premise.profile))?;
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
                    encoder.field("requirement_identity", |encoder| {
                        encoder.string(&route.requirement_identity)
                    })
                })
            })
        })
    })?;
    encoder.field("calling", |encoder| {
        encoder.option(method.calling.as_ref(), encode_application)
    })
}
