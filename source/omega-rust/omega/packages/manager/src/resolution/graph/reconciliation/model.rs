//! Paths, limits, conflicts, and failures exposed by reconciliation.

use super::super::validation::PackageClosureValidationError;
use crate::declarations::dependency_projection::DependencySourceRequest;
use crate::resolution::package::PackageSourceCustody;
use omega_package_source::{AliasName, PackageKey};
use std::fmt;

/// One exact requester-local edge in a root-to-dependency path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyRequestPathStep {
    pub(super) requester: PackageKey,
    pub(super) dependency_index: usize,
    pub(super) alias: AliasName,
    pub(super) target: PackageKey,
}

impl DependencyRequestPathStep {
    pub fn requester(&self) -> &PackageKey {
        &self.requester
    }

    /// Zero-based position in the requester's projected dependency rows.
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
    pub(super) root: PackageKey,
    pub(super) steps: Vec<DependencyRequestPathStep>,
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
    pub(super) custody: PackageSourceCustody,
    pub(super) requesting_paths: Vec<DependencyRequestPath>,
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
    pub(super) key: PackageKey,
    pub(super) candidates: Vec<PackageSourceClosureConflictCandidate>,
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
}

impl<E> PackageSourceClosureResolutionError<E> {
    pub fn conflicts(&self) -> Option<&[PackageSourceClosureConflict]> {
        match self {
            Self::ConflictingCustody { conflicts } => Some(conflicts),
            Self::Adapter { .. } | Self::LimitExceeded { .. } | Self::InvalidClosure { .. } => None,
        }
    }
}

impl<E: fmt::Display> fmt::Display for PackageSourceClosureResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adapter {
                requester,
                dependency_index,
                error,
                ..
            } => write!(
                formatter,
                "source adapter failed for dependency row {dependency_index} of package `{}`: {error}",
                requester.name().as_str()
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
            Self::InvalidClosure { errors } => write!(
                formatter,
                "resolved package source closure failed {} graph validation check(s)",
                errors.len()
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PackageSourceClosureResolutionError<E> {}
