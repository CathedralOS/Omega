//! Occurrence coverage derived from the canonical source closure.
//!
//! The roster is the join column between package-keyed records and the exact
//! authorized occurrences a package serves. Inside one closure the selected
//! target and build execution profile are section-level facts, so occurrence
//! identity reduces to (package, purpose): the root occurs at the product
//! purpose, a product edge propagates every purpose its requester holds, and
//! a build edge contributes the build purpose for its selected package.

use crate::declarations::PackageKey;
use crate::declarations::dependencies::DependencyPurpose;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use std::fmt;
use target::TargetProfile;

/// Explicit checked context metadata; retaining it grants no compilation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageCheckedContext {
    purpose: DependencyPurpose,
    target: TargetProfile,
    build_execution_profile: Option<TargetProfile>,
}

impl PackageCheckedContext {
    pub const fn new(
        purpose: DependencyPurpose,
        target: TargetProfile,
        build_execution_profile: Option<TargetProfile>,
    ) -> Self {
        Self {
            purpose,
            target,
            build_execution_profile,
        }
    }
    pub const fn purpose(self) -> DependencyPurpose {
        self.purpose
    }
    pub const fn target(self) -> TargetProfile {
        self.target
    }
    pub const fn build_execution_profile(self) -> Option<TargetProfile> {
        self.build_execution_profile
    }
}

/// One package's authorized occurrences: the exact purposes it serves for the
/// enclosing closure's target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePurposeCoverage {
    package: PackageKey,
    purposes: Vec<DependencyPurpose>,
}

impl PackagePurposeCoverage {
    pub const fn package(&self) -> &PackageKey {
        &self.package
    }

    /// Sorted nonempty purposes this package occurrence set covers.
    pub fn purposes(&self) -> &[DependencyPurpose] {
        &self.purposes
    }
}

/// The complete occurrence roster of one canonical source closure, in the
/// subject's canonical package order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageOccurrenceRoster {
    coverages: Vec<PackagePurposeCoverage>,
}

impl PackageOccurrenceRoster {
    /// Derive the roster by propagating purposes over the validated graph.
    /// Subjects only exist once validated as one reachable acyclic closure, so
    /// every package ends with at least one purpose; the check stays total
    /// rather than assuming it.
    pub fn derive(
        source: &CanonicalSourceClosureSubject,
    ) -> Result<Self, PackageOccurrenceRosterError> {
        let packages = source.packages();
        let mut purposes = Vec::new();
        purposes
            .try_reserve_exact(packages.len())
            .map_err(|_| PackageOccurrenceRosterError::AllocationFailed)?;
        purposes.resize(packages.len(), 0u8);
        let position = |key: &PackageKey| {
            packages
                .binary_search_by(|package| package.key().cmp(key))
                .map_err(|_| PackageOccurrenceRosterError::UnknownPackage)
        };
        let root = position(source.root().selected().key())?;
        purposes[root] = purpose_mask_bit(DependencyPurpose::Product);
        let mut pending = vec![root];
        while let Some(requester) = pending.pop() {
            let requests = source.dependency_requests();
            let start =
                requests.partition_point(|request| request.requester() < packages[requester].key());
            let end = start
                + requests[start..]
                    .partition_point(|request| request.requester() == packages[requester].key());
            for request in &requests[start..end] {
                let contribution = if request.purpose().is_product() {
                    purposes[requester]
                } else {
                    purpose_mask_bit(DependencyPurpose::Build)
                };
                let selected = position(request.selected().key())?;
                if purposes[selected] & contribution != contribution {
                    purposes[selected] |= contribution;
                    pending.push(selected);
                }
            }
        }
        let mut coverages = Vec::new();
        coverages
            .try_reserve_exact(packages.len())
            .map_err(|_| PackageOccurrenceRosterError::AllocationFailed)?;
        for (package, mask) in packages.iter().zip(purposes.iter()) {
            if *mask == 0 {
                return Err(PackageOccurrenceRosterError::UnreachablePackage);
            }
            coverages.push(PackagePurposeCoverage {
                package: package.key().clone(),
                purposes: DependencyPurpose::ALL
                    .iter()
                    .copied()
                    .filter(|purpose| mask & purpose_mask_bit(*purpose) != 0)
                    .collect(),
            });
        }
        Ok(Self { coverages })
    }

    /// Coverages in the subject's canonical package order.
    pub fn coverages(&self) -> &[PackagePurposeCoverage] {
        &self.coverages
    }

    /// The exact purposes one package occurs under, or none when the package
    /// is absent from the closure.
    pub fn purposes(&self, package: &PackageKey) -> Option<&[DependencyPurpose]> {
        self.coverages
            .binary_search_by(|coverage| coverage.package.cmp(package))
            .ok()
            .map(|index| self.coverages[index].purposes.as_slice())
    }

    /// Whether (package, purpose) is an authorized occurrence in the closure.
    pub fn is_occurrence(&self, package: &PackageKey, purpose: DependencyPurpose) -> bool {
        self.purposes(package)
            .is_some_and(|purposes| purposes.contains(&purpose))
    }

    /// Total (package, purpose) occurrences across the closure.
    pub fn occurrence_count(&self) -> usize {
        self.coverages
            .iter()
            .map(|coverage| coverage.purposes.len())
            .sum()
    }
}

/// A closed failure while deriving occurrence coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageOccurrenceRosterError {
    /// The subject names a package outside its own canonical package set, or
    /// leaves a package unreachable; validated subjects never exhibit either.
    UnknownPackage,
    UnreachablePackage,
    AllocationFailed,
}

impl fmt::Display for PackageOccurrenceRosterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnknownPackage | Self::UnreachablePackage => {
                "source closure does not yield a complete occurrence roster"
            }
            Self::AllocationFailed => "package occurrence roster allocation failed",
        })
    }
}

impl std::error::Error for PackageOccurrenceRosterError {}

/// Purposes encode as one bitmask in `DependencyPurpose::ALL` order.
pub(crate) fn purpose_mask_bit(purpose: DependencyPurpose) -> u8 {
    let index = DependencyPurpose::ALL
        .iter()
        .position(|candidate| *candidate == purpose)
        .expect("dependency purpose belongs to the complete purpose list");
    u8::try_from(index)
        .ok()
        .and_then(|index| 1u8.checked_shl(u32::from(index)))
        .expect("dependency purpose count fits one mask byte")
}
