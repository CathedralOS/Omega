/// One compiler-owned source-resolution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionPhase {
    /// Remote object-format and selector discovery through ordinary host Git.
    TransportDiscovery,
    /// Creation of a new local bare repository.
    RepositoryInitialization,
    /// Fetch into an existing quarantine through ordinary host Git.
    Fetch,
    /// Read-only object and tree inspection.
    RepositoryInspection,
}

impl ResolverExecutionPhase {
    pub(crate) const fn requires_mutable_root(self) -> bool {
        matches!(self, Self::RepositoryInitialization | Self::Fetch)
    }
}
