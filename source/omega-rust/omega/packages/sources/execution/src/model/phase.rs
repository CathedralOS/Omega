/// One compiler-owned source-resolution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionPhase {
    /// Remote object-format and selector discovery. The selected Git process
    /// and its host-selected descendants have ordinary host transport,
    /// authentication, execution, and writable-state authority.
    TransportDiscovery,
    /// Creation of a new local bare repository. Native policy may confine
    /// writes and network when that preserves selected executable behavior;
    /// otherwise those guarantees are reported unavailable.
    RepositoryInitialization,
    /// Fetch into an existing quarantine. The selected Git process and its
    /// host-selected descendants have ordinary host transport, authentication,
    /// execution, and writable-state authority; the mutable root identifies
    /// Omega's quarantine custody rather than an exclusive host-write grant.
    Fetch,
    /// Read-only object and tree inspection. Command construction requests no
    /// mutation or transport; optional native denial guarantees are reported
    /// according to the policy actually applied.
    RepositoryInspection,
}

impl ResolverExecutionPhase {
    pub(crate) const fn permits_network(self) -> bool {
        matches!(self, Self::TransportDiscovery | Self::Fetch)
    }

    pub(crate) const fn requires_mutable_root(self) -> bool {
        matches!(self, Self::RepositoryInitialization | Self::Fetch)
    }

    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::TransportDiscovery => 1,
            Self::RepositoryInitialization => 2,
            Self::Fetch => 3,
            Self::RepositoryInspection => 4,
        }
    }
}
