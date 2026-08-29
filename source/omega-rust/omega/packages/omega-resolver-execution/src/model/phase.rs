/// One compiler-owned source-resolution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionPhase {
    /// Remote object-format and selector discovery. Network is available; disk
    /// mutation is not.
    TransportDiscovery,
    /// Creation of a new local bare repository. Only the supplied mutable root
    /// is writable; network is unavailable.
    RepositoryInitialization,
    /// Fetch into an existing quarantine. Network and the supplied mutable root
    /// are available.
    Fetch,
    /// Read-only object and tree inspection. Neither network nor filesystem
    /// mutation is available.
    RepositoryInspection,
}

impl ResolverExecutionPhase {
    pub(crate) const fn permits_network(self) -> bool {
        matches!(self, Self::TransportDiscovery | Self::Fetch)
    }

    pub(crate) const fn requires_mutable_root(self) -> bool {
        matches!(self, Self::RepositoryInitialization | Self::Fetch)
    }

    pub(crate) const fn permits_descendant_processes(self) -> bool {
        self.permits_network()
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

/// Closed transport authority selected for a networked resolver phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionNetworkTransport {
    Https,
    Ssh,
}

impl ResolverExecutionNetworkTransport {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::Https => 1,
            Self::Ssh => 2,
        }
    }
}
