//! Portable, normalized service identities admitted on one dependency use.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, record_storage::Record)]
pub struct DependencyReach {
    services: Vec<String>,
}

impl DependencyReach {
    /// Callers supply the full canonical application identities and their normalized parents.
    pub fn from_checked_services(services: impl IntoIterator<Item = String>) -> Self {
        let mut services: Vec<_> = services.into_iter().collect();
        services.sort_unstable();
        services.dedup();
        Self { services }
    }

    pub fn admits(&self, service: &str) -> bool {
        self.services
            .binary_search_by(|candidate| candidate.as_str().cmp(service))
            .is_ok()
    }

    pub fn is_subset_of(&self, limit: &Self) -> bool {
        self.services.iter().all(|service| limit.admits(service))
    }

    pub fn services(&self) -> impl Iterator<Item = &str> {
        self.services.iter().map(String::as_str)
    }
}

/// One checked incoming edge in a particular activation. Anonymous Build roots have no package key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyServiceEdge {
    pub requester: Option<crate::PackageKeyIdentity>,
    pub package: crate::PackageKeyIdentity,
    pub alias: String,
    pub max_reach: DependencyReach,
}

/// The dependency selected at an emitted direct-call site. Package imports are
/// requester-local; the callable's canonical declaration alone cannot name this edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyCallOrigin {
    pub package: crate::PackageKeyIdentity,
    pub alias: String,
}

pub type DependencyCallOrigins = std::collections::BTreeMap<
    (crate::MachineId, crate::CallableInvocationSite),
    std::collections::BTreeSet<DependencyCallOrigin>,
>;
