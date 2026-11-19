//! Package selection permissions presented to a compilation.
#![forbid(unsafe_code)]

mod custody;
pub use custody::require_declaration_selection;

use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeMap;

pub trait DeclarationSelectionAuthority: Send + Sync {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool;

    fn package_label(&self, identity: PackageKeyIdentity) -> String;

    fn declaration_selection_scope(
        &self,
    ) -> Option<std::collections::BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
        None
    }
}

/// The direct Product dependencies available to calling-policy invocations.
/// There are no package resolvers or source files in this projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDeclarationScope(BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>);

impl ProductDeclarationScope {
    pub fn from_direct_dependencies(
        packages: &[PackageKeyIdentity],
        edges: impl IntoIterator<Item = (PackageKeyIdentity, PackageKeyIdentity)>,
    ) -> Self {
        let mut scopes = packages
            .iter()
            .map(|&package| (package, vec![package]))
            .collect::<BTreeMap<_, _>>();
        for (requester, owner) in edges {
            if scopes.contains_key(&owner)
                && let Some(owners) = scopes.get_mut(&requester)
            {
                owners.push(owner);
            }
        }
        for owners in scopes.values_mut() {
            owners.sort_unstable();
            owners.dedup();
        }
        Self(scopes)
    }
    pub fn into_requesters(self) -> BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>> {
        self.0
    }
    pub fn label(identity: PackageKeyIdentity) -> String {
        use std::fmt::Write as _;
        let mut label = String::with_capacity(72);
        label.push_str("package ");
        for byte in identity.digest() {
            let _ = write!(label, "{byte:02x}");
        }
        label
    }
}

impl DeclarationSelectionAuthority for ProductDeclarationScope {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        self.0
            .get(&requester)
            .is_some_and(|owners| owners.binary_search(&owner).is_ok())
    }
    fn package_label(&self, identity: PackageKeyIdentity) -> String {
        Self::label(identity)
    }
    fn declaration_selection_scope(
        &self,
    ) -> Option<BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
        Some(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_grants_only_listed_packages_and_their_direct_dependencies() {
        let package = |byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap();
        let (a, b, c, outsider) = (package(1), package(2), package(3), package(4));
        let scope = ProductDeclarationScope::from_direct_dependencies(
            &[a, b, c],
            [(a, b), (a, b), (b, c), (a, outsider), (outsider, b)],
        );
        assert!(scope.allows_declaration_selection(a, a));
        assert!(scope.allows_declaration_selection(a, b));
        assert!(!scope.allows_declaration_selection(a, c));
        assert!(!scope.allows_declaration_selection(a, outsider));
        assert!(!scope.allows_declaration_selection(outsider, b));
        assert_eq!(scope.into_requesters()[&a], [a, b]);
    }
}
