use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};

use crate::{CodecError, decode_crash_route_buckets, encode_crash_route_buckets};

fn route(cause: CrashCause) -> CrashRouteBucket {
    CrashRouteBucket {
        cause,
        alternatives: vec![CrashRouteGuard::Truth],
    }
}

#[test]
fn standalone_crash_route_roster_round_trips_and_rejects_corruption() {
    let routes = vec![route(CrashCause::Trap), route(CrashCause::Abort)];
    let encoded = encode_crash_route_buckets(&routes).unwrap();
    assert_eq!(decode_crash_route_buckets(&encoded).unwrap(), routes);

    let truncated = &encoded[..encoded.len() - 1];
    assert!(decode_crash_route_buckets(truncated).is_err());
}

#[test]
fn standalone_crash_route_roster_rejects_noncanonical_order() {
    let routes = vec![route(CrashCause::Abort), route(CrashCause::Trap)];
    assert_eq!(
        encode_crash_route_buckets(&routes),
        Err(CodecError::NonCanonicalOrder(
            "crash routes by cause and guard"
        ))
    );
}
