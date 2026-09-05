//! Normalized operational facts share the enclosing callable writer budget.

use super::super::values::{
    crashes::encode_boolean_expression, expressions::encode_contract_expression,
};
use super::*;
use crate::record::*;

pub(super) fn capability(
    encoder: &mut Encoder,
    flow: &PackagePolicyCapabilityFlow,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("capability", |encoder| {
        encode_nominal(encoder, &flow.capability)
    })?;
    encoder.field("kind", |encoder| {
        match flow.kind {
            flow_effects::CapabilityFlowKind::Uses => encoder.tag("uses", 0),
            flow_effects::CapabilityFlowKind::Returns => encoder.tag("returns", 1),
            flow_effects::CapabilityFlowKind::Acquires => encoder.tag("acquires", 2),
            flow_effects::CapabilityFlowKind::Stores => encoder.tag("stores", 3),
            flow_effects::CapabilityFlowKind::Derives => encoder.tag("derives", 4),
        }
        Ok(())
    })
}

pub(super) fn crash(
    encoder: &mut Encoder,
    crash: &PackagePolicyCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("interface", |encoder| {
        match crash.interface {
            PackageReviewCrashInterface::InternalInferred => encoder.tag("internal_inferred", 0),
            PackageReviewCrashInterface::PublishedCeiling => encoder.tag("published_ceiling", 1),
        }
        Ok(())
    })?;
    encoder.field("published", |encoder| {
        encoder.sequence(&crash.published, crash_route)
    })?;
    encoder.field("structural_runtime_requirements", |encoder| {
        encoder.option(
            crash.structural_runtime_requirements.as_deref(),
            |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
        )
    })?;
    encoder.field("inferred", |encoder| {
        match &crash.inferred {
            PackagePolicyInferredCrash::Unknown => encoder.tag("unknown", 0),
            PackagePolicyInferredCrash::Complete { causes } => {
                encoder.tag("complete", 1);
                encoder.field("causes", |encoder| {
                    encoder.sequence(causes, |encoder, cause| {
                        match cause {
                            PackageReviewCrashCause::Trap => encoder.tag("trap", 0),
                            PackageReviewCrashCause::Abort => encoder.tag("abort", 1),
                        }
                        Ok(())
                    })
                })?;
            }
        }
        Ok(())
    })
}

pub(in crate::encoding) fn crash_route(
    encoder: &mut Encoder,
    route: &PackagePolicyCrashRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("cause", |encoder| {
        match route.cause {
            PackageReviewCrashCause::Trap => encoder.tag("trap", 0),
            PackageReviewCrashCause::Abort => encoder.tag("abort", 1),
        }
        Ok(())
    })?;
    encoder.field("alternative_guards", |encoder| {
        encoder.sequence(&route.alternative_guards, |encoder, guard| {
            match guard {
                PackagePolicyCrashGuard::Truth => encoder.tag("truth", 0),
                PackagePolicyCrashGuard::Expression(expression) => {
                    encoder.tag("expression", 1);
                    encoder.field("expression", |encoder| {
                        encode_contract_expression(encoder, expression)
                    })?;
                }
            }
            Ok(())
        })
    })
}

pub(super) fn mutation(
    encoder: &mut Encoder,
    mutation: &PackagePolicyMutation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("completeness", |encoder| {
        match mutation.completeness {
            PackageReviewWriteFrameCompleteness::Complete => encoder.tag("complete", 0),
            PackageReviewWriteFrameCompleteness::Opaque => encoder.tag("opaque", 1),
        }
        Ok(())
    })?;
    encoder.field("paths", |encoder| {
        encoder.sequence(&mutation.paths, |encoder, path| encoder.string(path))
    })
}

pub(in crate::encoding) fn termination(
    encoder: &mut Encoder,
    termination: &PackagePolicyTermination,
) -> Result<(), PackageReviewEncodingError> {
    match termination {
        PackagePolicyTermination::NoGuarantee => encoder.tag("no_guarantee", 0),
        PackagePolicyTermination::Terminates { premises } => {
            encoder.tag("terminates", 1);
            encoder.field("premises", |encoder| encoder.sequence(premises, |encoder, premise| {
                encoder.field("profile", |encoder| encode_nominal(encoder, &premise.profile))?;
                encoder.field("subject", |encoder| { match &premise.subject {
                    PackageReviewProgressSubject::Declaration(identity) => {
                        encoder.tag("declaration", 0);
                        encoder.field("identity", |encoder| encode_nominal(encoder, identity))?;
                    }
                    PackageReviewProgressSubject::Receiver => encoder.tag("receiver", 1),
                    PackageReviewProgressSubject::Parameter(position) => {
                        encoder.tag("parameter", 2);
                        encoder.field("position", |encoder| { encoder.u32(*position); Ok(()) })?;
                    }
                } Ok(()) })?;
                encoder.field("projections", |encoder| encoder.sequence(&premise.projections, encode_nominal))?;
                encoder.field("establishment_routes", |encoder| encoder.sequence(&premise.establishment_routes, |encoder, route| {
                    use effects::provider_plan::ServiceProgressEstablishmentRouteKind as Kind;
                    encoder.field("kind", |encoder| {
                        match route.kind {
                            Kind::CheckedRequirement => encoder.tag("checked_requirement", 0),
                            Kind::BoundaryRequirement => encoder.tag("boundary_requirement", 1),
                        }
                        Ok(())
                    })?;
                    encoder.field("requirement_owner", |encoder| encode_nominal(encoder, &route.requirement_owner))?;
                    encoder.field("requirement", |encoder| encode_nominal(encoder, &route.requirement))
                }))
            }))?;
        }
    }
    Ok(())
}
