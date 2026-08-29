//! Bounded traversal and reconciliation of the complete package source closure.
//!
//! Source custody defines what was selected, resolution traverses every
//! declared request, and the resolved closure exposes the validated result.

mod model;
mod resolution;
mod resolved_closure;
mod source_custody;

pub use model::{
    DependencyRequestPath, DependencyRequestPathStep, PackageSourceClosureConflict,
    PackageSourceClosureConflictCandidate, PackageSourceClosureLimitKind,
    PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
pub use resolved_closure::{
    ResolvedDependencySourceRequest, ResolvedPackageSourceClosure, ResolvedPackageSourceRequestSet,
    ResolvedRootPackageSourceRequest,
};
pub use source_custody::{PackageRootSourceRequest, PackageSourceCustody};

#[cfg(test)]
pub(crate) use resolution::resolve_package_source_closure;
pub(crate) use resolution::resolve_package_source_closure_with_limits;

#[cfg(test)]
mod tests;
