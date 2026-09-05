//! Closed callable behavior vocabulary, without proof coordinates.

use super::super::{expressions::expression, structural_expressions::boolean_expression};
use super::*;

pub(super) fn capability(reader: &mut Reader<'_>) -> Result<PackagePolicyCapabilityFlow, Error> {
    Ok(PackagePolicyCapabilityFlow {
        capability: nominal(reader)?,
        kind: match reader.byte()? {
            0 => psi_effects::CapabilityFlowKind::Uses,
            1 => psi_effects::CapabilityFlowKind::Returns,
            2 => psi_effects::CapabilityFlowKind::Acquires,
            3 => psi_effects::CapabilityFlowKind::Stores,
            4 => psi_effects::CapabilityFlowKind::Derives,
            _ => return Err(Error::InvalidTag),
        },
    })
}

pub(super) fn crash(reader: &mut Reader<'_>) -> Result<PackagePolicyCrash, Error> {
    Ok(PackagePolicyCrash {
        interface: match reader.byte()? {
            0 => PackageReviewCrashInterface::InternalInferred,
            1 => PackageReviewCrashInterface::PublishedCeiling,
            _ => return Err(Error::InvalidTag),
        },
        published: reader.sequence(9, |reader| {
            Ok(PackagePolicyCrashRoute {
                cause: match reader.byte()? {
                    0 => PackageReviewCrashCause::Trap,
                    1 => PackageReviewCrashCause::Abort,
                    _ => return Err(Error::InvalidTag),
                },
                alternative_guards: reader.sequence(1, |reader| {
                    Ok(match reader.byte()? {
                        0 => PackagePolicyCrashGuard::Truth,
                        1 => PackagePolicyCrashGuard::Expression(expression(reader)?),
                        _ => return Err(Error::InvalidTag),
                    })
                })?,
            })
        })?,
        structural_runtime_requirements: reader
            .option(|reader| reader.sequence(1, boolean_expression))?,
        inferred: match reader.byte()? {
            0 => PackagePolicyInferredCrash::Unknown,
            1 => PackagePolicyInferredCrash::Complete {
                causes: reader.sequence(1, |reader| {
                    Ok(match reader.byte()? {
                        0 => PackageReviewCrashCause::Trap,
                        1 => PackageReviewCrashCause::Abort,
                        _ => return Err(Error::InvalidTag),
                    })
                })?,
            },
            _ => return Err(Error::InvalidTag),
        },
    })
}

pub(super) fn mutation(reader: &mut Reader<'_>) -> Result<PackagePolicyMutation, Error> {
    Ok(PackagePolicyMutation {
        completeness: match reader.byte()? {
            0 => PackageReviewWriteFrameCompleteness::Complete,
            1 => PackageReviewWriteFrameCompleteness::Opaque,
            _ => return Err(Error::InvalidTag),
        },
        paths: reader.sequence(8, |reader| reader.string())?,
    })
}

pub(super) fn termination(reader: &mut Reader<'_>) -> Result<PackagePolicyTermination, Error> {
    Ok(match reader.byte()? {
        0 => PackagePolicyTermination::NoGuarantee,
        1 => PackagePolicyTermination::Terminates { premises: reader.sequence(1, |reader| Ok(PackagePolicyProgressPremise {
            profile: nominal(reader)?,
            subject: match reader.byte()? {
                0 => PackageReviewProgressSubject::Declaration(nominal(reader)?),
                1 => PackageReviewProgressSubject::Receiver,
                2 => PackageReviewProgressSubject::Parameter(reader.u32()?),
                _ => return Err(Error::InvalidTag),
            },
            projections: reader.sequence(41, nominal)?,
            establishment_routes: reader.sequence(83, |reader| {
                use omega_effects::provider_plan::ServiceProgressEstablishmentRouteKind as Kind;
                Ok(PackagePolicyServiceProgressRoute {
                    kind: match reader.byte()? { 0 => Kind::CheckedRequirement, 1 => Kind::BoundaryRequirement, _ => return Err(Error::InvalidTag) },
                    requirement_owner: nominal(reader)?,
                    requirement: nominal(reader)?,
                })
            })?,
        }))? },
        _ => return Err(Error::InvalidTag),
    })
}
