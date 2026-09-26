//! Provider-supplied resource profiles: the stable, external and atomic
//! capabilities each normalized region supplies, and the validated profile
//! with its restriction to a child interval.

use crate::validate_resource_profile;
use crate::{AccessPlanDiagnostic, AtomicPermissions, BoundaryReach};

/// Stable operations supplied by one admitted resource region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StableCapability {
    None,
    Read,
    Write,
    ReadWrite,
}

impl StableCapability {
    pub(crate) const fn permits(self, read: bool, write: bool) -> bool {
        (!read || matches!(self, Self::Read | Self::ReadWrite))
            && (!write || matches!(self, Self::Write | Self::ReadWrite))
    }

    pub(crate) const fn any(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Repeated-observation behavior supplied for external reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalReadBehavior {
    None,
    Repeatable,
    Destructive,
}

/// Whether memory backing a region can still be written by a peer the
/// provider does not control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PeerWritability {
    /// The provider holds exclusive write custody: either exclusive from
    /// acquisition, or a formerly shared peer's write permission revoked
    /// and remapped with cross-core invalidation completed before the
    /// supply was offered. `Stable` supply over this region is coherent.
    Exclusive,
    /// A hostile writable peer can still rewrite these bytes. The region
    /// may honestly supply `External` or `Atomic` capability — every
    /// authorized access is one exact-width event under mutation — but
    /// never `Stable`: zero-copy stable placement would read what the peer
    /// can still change after validation. Content needing stable
    /// validation must first be copied into exclusive memory.
    HostileShared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransferRule {
    pub width_bits: u16,
    pub alignment_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExternalCapability {
    None,
    Access {
        read: ExternalReadBehavior,
        write: bool,
        transfers: Vec<TransferRule>,
    },
}

impl ExternalCapability {
    pub(crate) fn any(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicTransferRule {
    pub transfer: TransferRule,
    pub operations: AtomicPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AtomicCapability {
    None,
    Access { transfers: Vec<AtomicTransferRule> },
}

impl AtomicCapability {
    pub(crate) fn any(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// One provider-supplied relative interval. Regions are normalized and
/// disjoint; uncovered bytes intentionally supply no operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceRegion {
    pub offset: u64,
    pub length: u64,
    pub peer: PeerWritability,
    pub stable: StableCapability,
    pub external: ExternalCapability,
    pub atomic: AtomicCapability,
    pub reach: BoundaryReach,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ResourceProfile {
    pub regions: Vec<ResourceRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceProfileId(pub(crate) u64);

impl ResourceProfileId {
    /// Compact report/cache coordinate. Admission retains and replays the
    /// complete normalized resource profile rather than trusting this value.
    pub const fn compatibility_fingerprint(self) -> u64 {
        self.0
    }
}

/// Canonical provider supply over one relative range length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedResourceProfile {
    pub(crate) identity: ResourceProfileId,
    pub(crate) length: u64,
    pub(crate) regions: Vec<ResourceRegion>,
}

impl ValidatedResourceProfile {
    pub const fn identity(&self) -> ResourceProfileId {
        self.identity
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub fn regions(&self) -> &[ResourceRegion] {
        &self.regions
    }

    /// Intersect with a child interval, rebase retained regions to child zero,
    /// and attenuate every region's reach.
    pub fn restrict(
        &self,
        offset: u64,
        length: u64,
        permitted_reach: &BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        if length == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile restriction cannot be empty".into(),
            ));
        }
        let end = offset.checked_add(length).ok_or_else(|| {
            AccessPlanDiagnostic("resource-profile restriction range overflows".into())
        })?;
        if end > self.length {
            return Err(AccessPlanDiagnostic(format!(
                "resource-profile restriction {offset}..{end} exceeds {}-byte parent",
                self.length
            )));
        }
        let mut regions = Vec::new();
        for region in &self.regions {
            let region_end = region.offset + region.length;
            let start = region.offset.max(offset);
            let retained_end = region_end.min(end);
            if start >= retained_end {
                continue;
            }
            regions.push(ResourceRegion {
                offset: start - offset,
                length: retained_end - start,
                peer: region.peer,
                stable: region.stable,
                external: region.external.clone(),
                atomic: region.atomic.clone(),
                reach: region.reach.intersection(permitted_reach),
            });
        }
        validate_resource_profile(ResourceProfile { regions }, length)
    }
}
