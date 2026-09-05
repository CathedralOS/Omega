use super::{Error, expressions::expression, identity::nominal, reader::Reader};
use crate::record::{
    PackageReviewCrashCause, PackageReviewCrashPredicate, PackageReviewCrashRoute,
    PackageReviewCrashRouteGuard, PackageReviewProgressPremise, PackageReviewProgressSubject,
    PackageReviewSynchronousInvocation, PackageReviewTermination,
};

pub(super) fn synchronous_invocation(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewSynchronousInvocation, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewSynchronousInvocation::Parameter(reader.u32()?),
        1 => PackageReviewSynchronousInvocation::Service(nominal(reader)?),
        _ => return Err(Error::InvalidTag),
    })
}

pub(super) fn termination(reader: &mut Reader<'_>) -> Result<PackageReviewTermination, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewTermination::NoGuarantee,
        1 => PackageReviewTermination::Terminates {
            premises: reader.sequence(1, |reader| {
                let profile = nominal(reader)?;
                let subject = match reader.byte()? {
                    0 => PackageReviewProgressSubject::Declaration(nominal(reader)?),
                    1 => PackageReviewProgressSubject::Receiver,
                    2 => PackageReviewProgressSubject::Parameter(reader.u32()?),
                    _ => return Err(Error::InvalidTag),
                };
                Ok(PackageReviewProgressPremise {
                    profile,
                    subject,
                    projections: reader.sequence(41, nominal)?,
                })
            })?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

pub(super) fn crash_route(reader: &mut Reader<'_>) -> Result<PackageReviewCrashRoute, Error> {
    let cause = match reader.byte()? {
        0 => PackageReviewCrashCause::Trap,
        1 => PackageReviewCrashCause::Abort,
        _ => return Err(Error::InvalidTag),
    };
    let alternative_guards = reader.sequence(1, |reader| {
        Ok(match reader.byte()? {
            0 => PackageReviewCrashRouteGuard::Truth,
            1 => PackageReviewCrashRouteGuard::Predicate(PackageReviewCrashPredicate {
                canonical_bytes: reader.bytes()?,
            }),
            2 => PackageReviewCrashRouteGuard::Expression(expression(reader)?),
            _ => return Err(Error::InvalidTag),
        })
    })?;
    Ok(PackageReviewCrashRoute {
        cause,
        alternative_guards,
    })
}
