//! Closed callable behavior vocabulary, without proof coordinates.
use super::Error;
use crate::encoding::recovery::policy::identity::nominal;
use crate::encoding::recovery::policy::reader::Reader;
use crate::record::PackagePolicyCapabilityFlow;
use crate::record::PackagePolicyCrash;
use crate::record::PackagePolicyCrashGuard;
use crate::record::PackagePolicyCrashRoute;
use crate::record::PackagePolicyInferredCrash;
use crate::record::PackagePolicyMutation;
use crate::record::PackagePolicyProgressPremise;
use crate::record::PackagePolicyServiceProgressRoute;
use crate::record::PackagePolicyTermination;
use crate::record::PackageReviewCrashCause;
use crate::record::PackageReviewCrashInterface;
use crate::record::PackageReviewProgressSubject;
use crate::record::PackageReviewWriteFrameCompleteness;

use super::super::{expressions::expression, structural_expressions::boolean_expression};

pub(in crate::encoding::recovery) fn capability(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCapabilityFlow, Error> {
    Ok(PackagePolicyCapabilityFlow {
        capability: nominal(reader)?,
        kind: super::super::tag!(reader => {
            0 => flow_effects::CapabilityFlowKind::Uses,
            1 => flow_effects::CapabilityFlowKind::Returns,
            2 => flow_effects::CapabilityFlowKind::Acquires,
            3 => flow_effects::CapabilityFlowKind::Stores,
            4 => flow_effects::CapabilityFlowKind::Derives,
        }),
    })
}

pub(in crate::encoding::recovery) fn crash(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCrash, Error> {
    Ok(PackagePolicyCrash {
        interface: super::super::tag!(reader => {
            0 => PackageReviewCrashInterface::InternalInferred,
            1 => PackageReviewCrashInterface::PublishedCeiling,
        }),
        published: reader.sequence(9, crash_route)?,
        structural_runtime_requirements: reader
            .option(|reader| reader.sequence(1, boolean_expression))?,
        inferred: super::super::tag!(reader => {
            0 => PackagePolicyInferredCrash::Unknown,
            1 => PackagePolicyInferredCrash::Complete {
                causes: reader.sequence(1, |reader| {
                    Ok(super::super::tag!(reader => {
                        0 => PackageReviewCrashCause::Trap,
                        1 => PackageReviewCrashCause::Abort,
                    }))
                })?,
            },
        }),
    })
}

pub(in crate::encoding::recovery) fn crash_route(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyCrashRoute, Error> {
    Ok(PackagePolicyCrashRoute {
        cause: super::super::tag!(reader => {
            0 => PackageReviewCrashCause::Trap,
            1 => PackageReviewCrashCause::Abort,
        }),
        alternative_guards: reader.sequence(1, |reader| {
            Ok(super::super::tag!(reader => {
                0 => PackagePolicyCrashGuard::Truth,
                1 => PackagePolicyCrashGuard::Expression(expression(reader)?),
            }))
        })?,
    })
}

pub(in crate::encoding::recovery) fn mutation(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyMutation, Error> {
    Ok(PackagePolicyMutation {
        completeness: super::super::tag!(reader => {
            0 => PackageReviewWriteFrameCompleteness::Complete,
            1 => PackageReviewWriteFrameCompleteness::Opaque,
        }),
        paths: reader.sequence(8, |reader| reader.string())?,
    })
}

pub(in crate::encoding::recovery) fn termination(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyTermination, Error> {
    Ok(super::super::tag!(reader => {
        0 => PackagePolicyTermination::NoGuarantee,
        1 => PackagePolicyTermination::Terminates {
            premises: reader.sequence(1, |reader| {
                Ok(PackagePolicyProgressPremise {
                    profile: nominal(reader)?,
                    subject: super::super::tag!(reader => {
                        0 => PackageReviewProgressSubject::Declaration(nominal(reader)?),
                        2 => PackageReviewProgressSubject::Parameter(reader.u32()?),
                    }),
                    projections: reader.sequence(41, nominal)?,
                    establishment_routes: reader.sequence(83, |reader| {
                        use effects::provider_plan::ServiceProgressEstablishmentRouteKind as Kind;
                        Ok(PackagePolicyServiceProgressRoute {
                            kind: super::super::tag!(reader => {
                                0 => Kind::CheckedRequirement,
                                1 => Kind::BoundaryRequirement,
                            }),
                            requirement_owner: nominal(reader)?,
                            requirement: nominal(reader)?,
                        })
                    })?,
                })
            })?,
        },
    }))
}
