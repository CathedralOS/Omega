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
    encode_nominal(encoder, &flow.capability)?;
    encoder.byte(match flow.kind {
        psi_effects::CapabilityFlowKind::Uses => 0,
        psi_effects::CapabilityFlowKind::Returns => 1,
        psi_effects::CapabilityFlowKind::Acquires => 2,
        psi_effects::CapabilityFlowKind::Stores => 3,
        psi_effects::CapabilityFlowKind::Derives => 4,
    });
    Ok(())
}

pub(super) fn crash(
    encoder: &mut Encoder,
    crash: &PackagePolicyCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match crash.interface {
        PackageReviewCrashInterface::InternalInferred => 0,
        PackageReviewCrashInterface::PublishedCeiling => 1,
    });
    encoder.sequence(&crash.published, |encoder, route| {
        encoder.byte(match route.cause {
            PackageReviewCrashCause::Trap => 0,
            PackageReviewCrashCause::Abort => 1,
        });
        encoder.sequence(&route.alternative_guards, |encoder, guard| {
            match guard {
                PackagePolicyCrashGuard::Truth => encoder.byte(0),
                PackagePolicyCrashGuard::Expression(expression) => {
                    encoder.byte(1);
                    encode_contract_expression(encoder, expression)?;
                }
            }
            Ok(())
        })
    })?;
    encoder.option(
        crash.structural_runtime_requirements.as_deref(),
        |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
    )?;
    match &crash.inferred {
        PackagePolicyInferredCrash::Unknown => encoder.byte(0),
        PackagePolicyInferredCrash::Complete { causes } => {
            encoder.byte(1);
            encoder.sequence(causes, |encoder, cause| {
                encoder.byte(match cause {
                    PackageReviewCrashCause::Trap => 0,
                    PackageReviewCrashCause::Abort => 1,
                });
                Ok(())
            })?;
        }
    }
    Ok(())
}

pub(super) fn mutation(
    encoder: &mut Encoder,
    mutation: &PackagePolicyMutation,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match mutation.completeness {
        PackageReviewWriteFrameCompleteness::Complete => 0,
        PackageReviewWriteFrameCompleteness::Opaque => 1,
    });
    encoder.sequence(&mutation.paths, |encoder, path| encoder.string(path))
}

pub(super) fn termination(
    encoder: &mut Encoder,
    termination: &PackagePolicyTermination,
) -> Result<(), PackageReviewEncodingError> {
    match termination {
        PackagePolicyTermination::NoGuarantee => encoder.byte(0),
        PackagePolicyTermination::Terminates { premises } => {
            encoder.byte(1);
            encoder.sequence(premises, |encoder, premise| {
                encode_nominal(encoder, &premise.profile)?;
                match &premise.subject {
                    PackageReviewProgressSubject::Declaration(identity) => {
                        encoder.byte(0);
                        encode_nominal(encoder, identity)?;
                    }
                    PackageReviewProgressSubject::Receiver => encoder.byte(1),
                    PackageReviewProgressSubject::Parameter(position) => {
                        encoder.byte(2);
                        encoder.u32(*position);
                    }
                }
                encoder.sequence(&premise.projections, encode_nominal)?;
                encoder.sequence(&premise.establishment_routes, |encoder, route| {
                    use omega_effects::provider_plan::ServiceProgressEstablishmentRouteKind as Kind;
                    encoder.byte(match route.kind {
                        Kind::CheckedRequirement => 0,
                        Kind::BoundaryRequirement => 1,
                    });
                    encode_nominal(encoder, &route.requirement_owner)?;
                    encode_nominal(encoder, &route.requirement)
                })
            })?;
        }
    }
    Ok(())
}
