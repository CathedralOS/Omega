//! Bounded traversal and reconciliation of the complete package source closure.
//!
//! Source custody defines what was selected, resolution traverses every
//! declared request, and the resolved closure exposes the validated result.

mod dependency_paths;
mod resolution;
mod resolved_closure;

pub(crate) use dependency_paths::DependencyRequestPaths;

pub use super::root_request::PackageRootSourceRequest;
pub use resolved_closure::{
    ExactTargetPackageSourceClosure, ResolvedDependencySourceRequest, ResolvedPackageSourceClosure,
    ResolvedPackageSourceRequestSet, ResolvedRootPackageSourceRequest,
};

use super::PackageClosureValidationError;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::{
    DependencyAliasError, DependencyPurpose, DependencySourceRequest,
};
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::source::PackageSourceCustody;
#[cfg(test)]
pub(crate) use resolution::resolve_package_source_closure;
pub(crate) use resolution::resolve_package_source_closure_with_indexed_limits;
pub(crate) use resolution::resolve_package_source_closure_with_limits;
use std::fmt;

#[cfg(test)]
mod tests;

/// One exact requester-local edge in a root-to-dependency path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequestPathStep {
    requester: PackageKey,
    purpose: DependencyPurpose,
    dependency_index: usize,
    alias: AliasName,
    target: PackageKey,
}

impl DependencyRequestPathStep {
    pub fn requester(&self) -> &PackageKey {
        &self.requester
    }

    /// Which authorized context this edge belongs to.
    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

    /// Zero-based position in the requester's projected dependency rows for
    /// this edge's purpose scope.
    pub fn dependency_index(&self) -> usize {
        self.dependency_index
    }

    pub fn alias(&self) -> &AliasName {
        &self.alias
    }

    pub fn target(&self) -> &PackageKey {
        &self.target
    }
}

/// One exact path by which source resolution discovered a package custody.
///
/// The root custody has an empty `steps` sequence. Dependency-row ordinals
/// keep repeated otherwise-identical authored requests distinguishable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequestPath {
    root: PackageKey,
    steps: Vec<DependencyRequestPathStep>,
}

impl DependencyRequestPath {
    pub fn root(&self) -> &PackageKey {
        &self.root
    }

    pub fn steps(&self) -> &[DependencyRequestPathStep] {
        &self.steps
    }
}

/// One distinct custody observed for a conflicted `PackageKey`, together with
/// every dependency path that produced that exact custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceClosureConflictCandidate {
    custody: PackageSourceCustody,
    requesting_paths: Vec<DependencyRequestPath>,
}

impl PackageSourceClosureConflictCandidate {
    pub fn custody(&self) -> &PackageSourceCustody {
        &self.custody
    }

    pub fn requesting_paths(&self) -> &[DependencyRequestPath] {
        &self.requesting_paths
    }
}

/// All distinct source custodies observed for one conflicting package key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceClosureConflict {
    key: PackageKey,
    candidates: Vec<PackageSourceClosureConflictCandidate>,
}

impl PackageSourceClosureConflict {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn candidates(&self) -> &[PackageSourceClosureConflictCandidate] {
        &self.candidates
    }
}

/// Resolver-work ceilings applied across one complete source closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageSourceClosureLimits {
    pub max_packages: usize,
    pub max_dependency_requests: usize,
    pub max_depth: usize,
}

impl Default for PackageSourceClosureLimits {
    fn default() -> Self {
        Self {
            max_packages: 1024,
            max_dependency_requests: 16 * 1024,
            max_depth: 128,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceClosureLimitKind {
    Packages,
    DependencyRequests,
    Depth,
}

#[derive(Debug)]
pub enum PackageSourceClosureResolutionError<E> {
    /// The adapter could not resolve one projected dependency request.
    Adapter {
        requester: PackageKey,
        purpose: DependencyPurpose,
        dependency_index: usize,
        request: DependencySourceRequest,
        error: E,
    },
    LimitExceeded {
        kind: PackageSourceClosureLimitKind,
        limit: usize,
    },
    /// One or more package keys produced non-identical immutable custody.
    ConflictingCustody {
        conflicts: Vec<PackageSourceClosureConflict>,
    },
    /// Final exact graph validation rejected the fully traversed closure.
    InvalidClosure {
        errors: Vec<PackageClosureValidationError>,
    },
    /// A dependency adapter returned a source whose authored role is not
    /// importable. Applications may be selected only as the closure root.
    InvalidDependencyRole {
        requester: PackageKey,
        purpose: DependencyPurpose,
        dependency_index: usize,
        selected: PackageKey,
        role: BuildDeclarationKind,
    },
    /// Requester-local aliases conflict after package-authored names have been
    /// recovered from source custody.
    InvalidAliases {
        requester: PackageKey,
        purpose: DependencyPurpose,
        error: DependencyAliasError,
    },
}

impl<E> PackageSourceClosureResolutionError<E> {
    pub fn conflicts(&self) -> Option<&[PackageSourceClosureConflict]> {
        match self {
            Self::ConflictingCustody { conflicts } => Some(conflicts),
            Self::Adapter { .. }
            | Self::LimitExceeded { .. }
            | Self::InvalidClosure { .. }
            | Self::InvalidDependencyRole { .. }
            | Self::InvalidAliases { .. } => None,
        }
    }
}

impl<E: fmt::Display> fmt::Display for PackageSourceClosureResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adapter {
                requester,
                purpose,
                dependency_index,
                error,
                ..
            } => write!(
                formatter,
                "source adapter failed for {purpose_name} dependency row {dependency_index} of package `{requester_name}`: {error}",
                purpose_name = purpose.name(),
                requester_name = requester.name().as_str(),
            ),
            Self::LimitExceeded { kind, limit } => write!(
                formatter,
                "package source closure exceeded its {kind:?} limit of {limit}"
            ),
            Self::ConflictingCustody { conflicts } => write!(
                formatter,
                "source closure contains conflicting custody for {} package key(s)",
                conflicts.len()
            ),
            Self::InvalidClosure { errors } => {
                write!(
                    formatter,
                    "resolved package source closure failed {} graph validation check(s)",
                    errors.len()
                )?;
                if let Some(cycle) = errors.iter().find_map(|error| match error {
                    PackageClosureValidationError::DependencyCycle { cycle } => Some(cycle),
                    _ => None,
                }) {
                    formatter.write_str("; dependency cycle: ")?;
                    for (position, package) in cycle.iter().take(16).enumerate() {
                        if position != 0 {
                            formatter.write_str(" -> ")?;
                        }
                        formatter.write_str(package.name().as_str())?;
                    }
                    if cycle.len() > 16 {
                        formatter.write_str(" -> ...")?;
                    }
                }
                Ok(())
            }
            Self::InvalidDependencyRole {
                requester,
                purpose,
                dependency_index,
                selected,
                role,
            } => write!(
                formatter,
                "{} dependency row {dependency_index} of package `{}` selected `{}` with non-package role {role:?}",
                purpose.name(),
                requester.name().as_str(),
                selected.name().as_str(),
            ),
            Self::InvalidAliases {
                requester,
                purpose,
                error,
            } => write!(
                formatter,
                "{} dependencies of package `{}` have invalid aliases: {error}",
                purpose.name(),
                requester.name().as_str(),
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PackageSourceClosureResolutionError<E> {}
