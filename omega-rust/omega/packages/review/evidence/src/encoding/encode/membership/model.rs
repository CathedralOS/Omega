use psi_core::PackageKeyIdentity;

/// Lowerable work and requested-scratch ceilings for semantic owner descent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyMembershipLimits {
    pub(super) maximum_owned_bytes: usize,
    pub(super) maximum_identity_nodes: usize,
    pub(super) maximum_depth: usize,
}

impl PackagePolicyMembershipLimits {
    pub const fn new(
        maximum_owned_bytes: usize,
        maximum_identity_nodes: usize,
        maximum_depth: usize,
    ) -> Self {
        Self {
            maximum_owned_bytes,
            maximum_identity_nodes,
            maximum_depth,
        }
    }
    pub const fn maximum_owned_bytes(self) -> usize {
        self.maximum_owned_bytes
    }
    pub const fn maximum_identity_nodes(self) -> usize {
        self.maximum_identity_nodes
    }
    pub const fn maximum_depth(self) -> usize {
        self.maximum_depth
    }
    pub(super) fn bounded(self) -> Self {
        Self::new(
            self.maximum_owned_bytes.min(64 * 1024 * 1024),
            self.maximum_identity_nodes.min(1_048_576),
            self.maximum_depth.min(128),
        )
    }
}

impl Default for PackagePolicyMembershipLimits {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024, 1_048_576, 128)
    }
}

/// Plain aggregate charges, not authority. Debit both before the next baseline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackagePolicyMembershipUsage {
    pub(super) owned_bytes: usize,
    pub(super) identity_nodes: usize,
}
impl PackagePolicyMembershipUsage {
    pub const fn owned_bytes(self) -> usize {
        self.owned_bytes
    }
    pub const fn identity_nodes(self) -> usize {
        self.identity_nodes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyMembershipError {
    UnknownPackage { package: PackageKeyIdentity },
    OwnedBytesLimitExceeded,
    IdentityNodeLimitExceeded,
    NestingLimitExceeded,
    MalformedIdentity,
    AllocationFailed,
    InvalidPolicy,
}
impl std::fmt::Display for PackagePolicyMembershipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "package policy membership: {self:?}")
    }
}
impl std::error::Error for PackagePolicyMembershipError {}
