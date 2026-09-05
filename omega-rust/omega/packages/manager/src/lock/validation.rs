//! Exact transitive source membership, with meaning interpreted by evidence.

use super::{PackageLockError as Error, PackageLockTarget};
use omega_package_evidence::encoding::PackagePolicyMembershipLimits;
use psi_core::PackageKeyIdentity;

pub(super) struct Usage {
    pub(super) owned_bytes: usize,
    pub(super) identity_nodes: usize,
}

pub(super) fn policy_source_membership(
    target: &PackageLockTarget,
    maximum_owned_bytes: usize,
    maximum_identity_nodes: usize,
) -> Result<Usage, Error> {
    // PackageKey is source/name ordered, not digest ordered. One bounded index
    // avoids rehashing every source key for every nested policy reference.
    let index_bytes = target
        .source()
        .packages()
        .len()
        .checked_mul(std::mem::size_of::<(PackageKeyIdentity, usize)>())
        .ok_or(Error::AllocationLimitExceeded)?;
    let mut remaining_owned = maximum_owned_bytes
        .checked_sub(index_bytes)
        .ok_or(Error::AllocationLimitExceeded)?;
    let mut remaining_nodes = maximum_identity_nodes;
    let mut packages = Vec::new();
    packages
        .try_reserve_exact(target.source().packages().len())
        .map_err(|_| Error::AllocationFailed)?;
    packages.extend(
        target
            .source()
            .packages()
            .iter()
            .enumerate()
            .map(|(index, source)| (source.key().identity(), index)),
    );
    packages.sort_unstable_by_key(|(identity, _)| *identity);
    let lookup = |identity| {
        packages
            .binary_search_by_key(&identity, |(package, _)| *package)
            .ok()
            .map(|index| packages[index].1)
    };
    for baseline in target.baselines() {
        let usage = baseline
            .validate_package_membership(
                |identity| lookup(identity).is_some(),
                PackagePolicyMembershipLimits::new(remaining_owned, remaining_nodes, 128),
            )
            .map_err(Error::PolicySourceMembership)?;
        remaining_owned = remaining_owned
            .checked_sub(usage.owned_bytes())
            .ok_or(Error::AllocationLimitExceeded)?;
        remaining_nodes = remaining_nodes
            .checked_sub(usage.identity_nodes())
            .ok_or(Error::CountLimitExceeded)?;
        baseline
            .validate_boundary_application_owners(|identity| {
                lookup(identity).map(|index| &target.baselines()[index])
            })
            .map_err(|_| Error::BoundaryApplicationMismatch)?;
    }
    Ok(Usage {
        owned_bytes: maximum_owned_bytes - remaining_owned,
        identity_nodes: maximum_identity_nodes - remaining_nodes,
    })
}
