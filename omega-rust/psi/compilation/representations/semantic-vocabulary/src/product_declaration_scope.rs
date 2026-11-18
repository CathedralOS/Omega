use crate::{PackageKeyIdentity, compiler_evaluation::DeclarationSelectionAuthority};
use std::collections::BTreeMap;

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
