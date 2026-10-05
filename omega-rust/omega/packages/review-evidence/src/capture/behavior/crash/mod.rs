//! Crash plans, routes, expressions, and permission frontiers.

mod expressions;
mod permissions;

use crate::record::{
    PackageReviewCrashCause, PackageReviewCrashRoute, PackageReviewCrashRouteGuard,
};

pub(super) use expressions::project_boolean_expression;
use permissions::project_crash_predicate;

pub(crate) fn project_crash_routes(
    routes: &[checked_trees::CrashRouteBucket],
) -> Vec<PackageReviewCrashRoute> {
    let mut projected = routes
        .iter()
        .map(|route| PackageReviewCrashRoute {
            cause: project_crash_cause(route.cause()),
            alternative_guards: route
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => PackageReviewCrashRouteGuard::Truth,
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        PackageReviewCrashRouteGuard::Predicate(project_crash_predicate(predicate))
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

pub(crate) const fn project_crash_cause(
    cause: checked_trees::CrashCause,
) -> PackageReviewCrashCause {
    match cause {
        checked_trees::CrashCause::Trap => PackageReviewCrashCause::Trap,
        checked_trees::CrashCause::Abort => PackageReviewCrashCause::Abort,
    }
}
