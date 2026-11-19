use crate::{AtomicPermissions, BoundaryReach};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StableCapability {
    None,
    Read,
    Write,
    ReadWrite,
}

impl StableCapability {
    pub const fn permits(self, read: bool, write: bool) -> bool {
        (!read || matches!(self, Self::Read | Self::ReadWrite))
            && (!write || matches!(self, Self::Write | Self::ReadWrite))
    }

    pub const fn any(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalReadBehavior {
    None,
    Repeatable,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PeerWritability {
    Exclusive,
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
    pub fn any(&self) -> bool {
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
    pub fn any(&self) -> bool {
        !matches!(self, Self::None)
    }
}

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
