//! Projection of selected operator crash expressions to package review format.
//!
//! An operator crash site is an invocation row, not a machine call: it keeps
//! the exact selected operator symbol and the caller state/statement where the
//! use occurred, and never borrows ordinary call coordinates. An empty
//! surviving bucket is meaningful evidence — the checker proved the route
//! discharged — so projection must not synthesize routes for it or drop the
//! site.

use crate::record::{PackageReviewCrashRoute, PackageReviewCrashRouteGuard};
use compiler::CheckedCompilation;

pub(super) fn project_operator_crash_routes(
    _compilation: &CheckedCompilation,
    routes: &[checked_trees::CrashRouteBucket],
) -> Vec<PackageReviewCrashRoute> {
    let mut projected = routes
        .iter()
        .map(project_operator_crash_route)
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn project_operator_crash_route(
    route: &checked_trees::CrashRouteBucket,
) -> PackageReviewCrashRoute {
    PackageReviewCrashRoute {
        cause: super::project_crash_cause(route.cause()),
        alternative_guards: route
            .alternative_guards()
            .iter()
            .map(project_operator_crash_guard)
            .collect(),
    }
}

fn project_operator_crash_guard(
    guard: &checked_trees::CrashRouteGuard,
) -> PackageReviewCrashRouteGuard {
    match guard {
        checked_trees::CrashRouteGuard::Truth => PackageReviewCrashRouteGuard::Truth,
        checked_trees::CrashRouteGuard::Predicate(predicate) => {
            // For selected operators, use the existing runtime predicate bytes
            // projection. Structural expression projection can be added later
            // when contract expression support is extended.
            PackageReviewCrashRouteGuard::Predicate(super::permissions::project_crash_predicate(
                predicate,
            ))
        }
    }
}
