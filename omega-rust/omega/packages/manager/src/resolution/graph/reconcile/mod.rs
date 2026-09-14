//! Bounded traversal and reconciliation of the complete package source closure.
//!
//! Source custody defines what was selected, resolution traverses every
//! declared request, and the resolved closure exposes the validated result.

mod dependency_paths;
mod model;
mod resolution;
mod resolved_closure;

pub(crate) use dependency_paths::DependencyRequestPaths;

pub use super::root_request::PackageRootSourceRequest;
pub use model::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
pub use resolved_closure::{
    ExactTargetPackageSourceClosure, ResolvedDependencySourceRequest, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest,
};

#[cfg(test)]
pub(crate) use resolution::resolve_package_source_closure;
pub(crate) use resolution::resolve_package_source_closure_with_indexed_limits;
pub(crate) use resolution::resolve_package_source_closure_with_limits;

#[cfg(test)]
mod tests;
