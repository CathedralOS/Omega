/*
    We perform bounded traversal and reconciliation of the complete package
    source closure. Source custody defines what was selected, resolution
    traverses every declared request, and the resolved closure exposes the
    validated result.

    Dependency-row ordinals keep repeated otherwise-identical authored requests
    distinguishable along root-to-dependency explanation paths. Applications may
    be selected only as the closure root; non-package dependency roles,
    conflicting custodies for a single `PackageKey`, alias collisions, and
    exceeded package/request/depth ceilings all fail closed.
*/

/*
    Source-closure reconciliation belongs to the package graph layer
    (`resolution/graph/reconcile/mod.rs`).
*/

mod dependency_paths;
mod resolution;
mod resolved_closure;

pub(crate) use super::root_request::PackageRootSourceRequest;
pub(crate) use dependency_paths::DependencyRequestPaths;

pub(crate) use resolved_closure::{ExactTargetPackageSourceClosure, ResolvedPackageSourceClosure};

use super::PackageClosureValidationError;
use crate::declarations::BuildDeclarationKind;
#[cfg(test)]
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::declarations::dependencies::read::{DependencyAliasError, DependencyPurpose};
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::source::PackageSourceCustody;
pub(crate) use resolution::{PendingSourceResolution, SourceClosureStep, begin_source_closure};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyRequestPathStep {
    requester: PackageKey,
    purpose: DependencyPurpose,
    dependency_index: usize,
    alias: AliasName,
    target: PackageKey,
}

impl DependencyRequestPathStep {
    pub fn alias(&self) -> &AliasName {
        &self.alias
    }

    pub fn target(&self) -> &PackageKey {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DependencyRequestPath {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageSourceClosureConflictCandidate {
    custody: PackageSourceCustody,
    requesting_paths: Vec<DependencyRequestPath>,
}

impl PackageSourceClosureConflictCandidate {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageSourceClosureConflict {
    key: PackageKey,
    candidates: Vec<PackageSourceClosureConflictCandidate>,
}

impl PackageSourceClosureConflict {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PackageSourceClosureLimits {
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
pub(crate) enum PackageSourceClosureLimitKind {
    Packages,
    DependencyRequests,
    Depth,
}

#[derive(Debug)]
pub(crate) enum PackageSourceClosureResolutionError<E> {
    Adapter {
        requester: PackageKey,
        purpose: DependencyPurpose,
        dependency_index: usize,
        #[cfg(test)]
        request: DependencySourceRequest,
        error: E,
    },
    LimitExceeded {
        kind: PackageSourceClosureLimitKind,
        limit: usize,
    },
    ConflictingCustody {
        conflicts: Vec<PackageSourceClosureConflict>,
    },
    InvalidClosure {
        errors: Vec<PackageClosureValidationError>,
    },
    InvalidDependencyRole {
        requester: PackageKey,
        purpose: DependencyPurpose,
        dependency_index: usize,
        selected: PackageKey,
        role: BuildDeclarationKind,
    },
    InvalidAliases {
        requester: PackageKey,
        purpose: DependencyPurpose,
        error: DependencyAliasError,
    },
}

impl<E> PackageSourceClosureResolutionError<E> {}

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
