//! Existing replay-bearing crash rows retain their established binary grammar.

use super::*;

pub(crate) fn encode_crash(
    encoder: &mut Encoder,
    crash: &PackageReviewCrash,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match crash.interface {
        PackageReviewCrashInterface::InternalInferred => 0,
        PackageReviewCrashInterface::PublishedCeiling => 1,
    });
    encoder.sequence(&crash.published, encode_crash_route)?;
    encoder.option(
        crash.structural_runtime_requirements.as_deref(),
        |encoder, requirements| encoder.sequence(requirements, encode_boolean_expression),
    )?;
    encoder.sequence(&crash.checked_sites, encode_crash_site)?;
    encoder.sequence(&crash.checked_calls, encode_crash_call)
}

pub(crate) fn encode_crash_route(
    encoder: &mut Encoder,
    route: &PackageReviewCrashRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.cause {
        PackageReviewCrashCause::Trap => 0,
        PackageReviewCrashCause::Abort => 1,
    });
    encoder.sequence(&route.alternative_guards, |encoder, guard| {
        match guard {
            PackageReviewCrashRouteGuard::Truth => encoder.byte(0),
            PackageReviewCrashRouteGuard::Predicate(predicate) => {
                encoder.byte(1);
                encoder.bytes(&predicate.canonical_bytes)?;
            }
            PackageReviewCrashRouteGuard::Expression(expression) => {
                encoder.byte(2);
                encode_contract_expression(encoder, expression)?;
            }
        }
        Ok(())
    })
}

pub(crate) fn encode_crash_site(
    encoder: &mut Encoder,
    site: &PackageReviewCrashSite,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &site.state)?;
    encoder.u32(site.statement_ordinal);
    encoder.byte(match site.cause {
        PackageReviewCrashCause::Trap => 0,
        PackageReviewCrashCause::Abort => 1,
    });
    encoder.sequence(&site.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&site.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&site.guard_covering_buckets, |encoder, bucket| {
        encoder.u32(*bucket);
        Ok(())
    })?;
    encoder.sequence(&site.frontier_lower_bound, encode_permission_claim)
}

pub(crate) fn encode_crash_predicate(
    encoder: &mut Encoder,
    predicate: &PackageReviewCrashPredicate,
) -> Result<(), PackageReviewEncodingError> {
    encoder.bytes(&predicate.canonical_bytes)
}

pub(crate) fn encode_permission_claim(
    encoder: &mut Encoder,
    claim: &PackageReviewPermissionClaim,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &claim.machine)?;
    encode_nominal(encoder, &claim.state)?;
    match &claim.source {
        PackageReviewPermissionSource::StateEntry => encoder.byte(0),
        PackageReviewPermissionSource::Statement { statement_ordinal } => {
            encoder.byte(1);
            encoder.u64(*statement_ordinal);
        }
        PackageReviewPermissionSource::Call {
            statement_ordinal,
            call_ordinal,
            target,
        } => {
            encoder.byte(2);
            encoder.u64(*statement_ordinal);
            encoder.u64(*call_ordinal);
            encode_nominal(encoder, target)?;
        }
        PackageReviewPermissionSource::StateExit => encoder.byte(3),
    }
    encoder.u32(claim.ordinal);
    Ok(())
}

pub(crate) fn encode_crash_call(
    encoder: &mut Encoder,
    call: &PackageReviewCrashCall,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &call.state)?;
    encoder.u32(call.statement_ordinal);
    encoder.u32(call.call_ordinal);
    encode_nominal(encoder, &call.target_machine)?;
    encode_nominal(encoder, &call.target_state)?;
    encoder.sequence(&call.path_guard_conjuncts, encode_crash_predicate)?;
    encoder.sequence(&call.path_guard_consequences, encode_crash_predicate)?;
    encoder.sequence(&call.surviving_buckets, encode_crash_route)
}
