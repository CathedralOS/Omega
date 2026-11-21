//! Canonical crash causes and guards shared by callable contracts and native identities.
use crate::CodecError;
use crate::fields::{Field, List, wire_enum, wire_record};
use crate::terms::order::validate_canonical_proposition;
use crate::terms::proposition::{decode_proposition, encode_proposition};
use crate::wire::{Reader, Writer, decode_canonical, strictly_increasing};
use terminal_psi::{CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard};

wire_record! {
        Bucket: CrashRouteBucket { cause: Cause, alternatives: List<RouteGuard> }
}

wire_enum! {
        Cause: CrashCause, "CrashCause" { 1 => Trap, 2 => Abort }
        RouteGuard: CrashRouteGuard, "CrashRouteGuard" {
            0 => Truth,
            1 => Predicate(predicate: CrashPredicate),
        }
}

pub struct CrashPredicate;
impl Field<CrashPredicateTerm> for CrashPredicate {
    fn write(writer: &mut Writer, value: &CrashPredicateTerm) -> Result<(), CodecError> {
        encode_proposition(writer, value.proposition())
    }
    fn read(reader: &mut Reader<'_>) -> Result<CrashPredicateTerm, CodecError> {
        Ok(CrashPredicateTerm::new(decode_proposition(reader)?))
    }
}

pub fn crash_routes_are_canonical(routes: &[CrashRouteBucket]) -> bool {
    strictly_increasing(routes.iter().map(|bucket| bucket.cause))
        && routes.iter().all(|bucket| {
            !bucket.alternatives.is_empty()
                && strictly_increasing(bucket.alternatives.iter())
                && (!bucket.alternatives.contains(&CrashRouteGuard::Truth)
                    || bucket.alternatives == [CrashRouteGuard::Truth])
        })
}

pub fn validate_crash_route_predicates(routes: &[CrashRouteBucket]) -> Result<(), CodecError> {
    for predicate in routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate),
        })
    {
        validate_canonical_proposition(predicate.proposition())?;
    }
    Ok(())
}

pub fn encode_crash_route_buckets(
    routes: &[terminal_psi::CrashRouteBucket],
) -> Result<Vec<u8>, CodecError> {
    if !crash_routes_are_canonical(routes) {
        return Err(CodecError::NonCanonicalOrder(
            "crash routes by cause and guard",
        ));
    }
    validate_crash_route_predicates(routes)?;
    let mut writer = Writer::default();
    writer.counted("crash route buckets", routes, Bucket::write)?;
    Ok(writer.finish())
}

pub fn decode_crash_route_buckets(
    bytes: &[u8],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, CodecError> {
    decode_canonical(bytes, List::<Bucket>::read, |routes| {
        encode_crash_route_buckets(routes)
    })
}
