//! Direct package permissions for choosing declarations during compilation.
#![forbid(unsafe_code)]

mod custody;
pub use custody::require_declaration_selection;

use record_storage::{Record, RecordError, RecordReader, RecordWriter};
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeMap;
use std::sync::Arc;

pub trait DeclarationSelectionAuthority: Send + Sync {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool;
    fn package_label(&self, identity: PackageKeyIdentity) -> String;
}

/// Each requester can name its own declarations and those of its direct dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDeclarationScope {
    owners: BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>,
    // Names belong to the current package graph and only affect diagnostics.
    names: Option<BTreeMap<PackageKeyIdentity, String>>,
}

pub use PackageDeclarationScope as ProductDeclarationScope;

impl PackageDeclarationScope {
    pub fn from_direct_dependencies<'a>(
        packages: impl IntoIterator<Item = &'a PackageKeyIdentity>,
        edges: impl IntoIterator<Item = (PackageKeyIdentity, PackageKeyIdentity)>,
    ) -> Self {
        let mut owners = packages
            .into_iter()
            .map(|&package| (package, vec![package]))
            .collect::<BTreeMap<_, _>>();
        for (requester, owner) in edges {
            if owners.contains_key(&owner)
                && let Some(row) = owners.get_mut(&requester)
            {
                row.push(owner);
            }
        }
        for row in owners.values_mut() {
            row.sort_unstable();
            row.dedup();
        }
        Self {
            owners,
            names: None,
        }
    }

    pub fn with_package_names(mut self, names: BTreeMap<PackageKeyIdentity, String>) -> Self {
        self.names = Some(names);
        self
    }

    pub fn require_current(
        previous: Option<&Self>,
        current: Option<&Self>,
        requesters: impl IntoIterator<Item = PackageKeyIdentity>,
    ) -> Result<(), &'static str> {
        match (previous, current) {
            (None, None) => Ok(()),
            (Some(previous), Some(current)) => {
                for requester in requesters {
                    let old = previous
                        .owners
                        .get(&requester)
                        .ok_or("retained declaration scope does not cover a requesting package")?;
                    let new = current
                        .owners
                        .get(&requester)
                        .ok_or("current declaration scope does not cover a requesting package")?;
                    if new != old {
                        return Err("requesting package declaration scope changed");
                    }
                }
                Ok(())
            }
            _ => Err("presence of declaration selection authority changed"),
        }
    }

    pub fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        self.owners
            .get(&requester)
            .is_some_and(|owners| owners.binary_search(&owner).is_ok())
    }

    /// Existing source checkpoints encode a 32-bit tag followed by the requesters' direct grants.
    pub fn write_checkpoint(
        scope: Option<&Self>,
        out: &mut RecordWriter,
    ) -> Result<(), RecordError> {
        out.record(|out| match scope {
            None => 0u32.write(out),
            Some(scope) => {
                1u32.write(out)?;
                scope.owners.write(out)
            }
        })
    }

    pub fn read_checkpoint<C>(
        input: &mut RecordReader<'_, C>,
    ) -> Result<Option<Arc<Self>>, RecordError> {
        input.record(|input| match u32::read(input)? {
            0 => Ok(None),
            1 => {
                let owners: BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>> =
                    Record::read(input)?;
                for (requester, row) in &owners {
                    if row.windows(2).any(|pair| pair[0] >= pair[1])
                        || row.binary_search(requester).is_err()
                        || row.iter().any(|owner| !owners.contains_key(owner))
                    {
                        return Err(input.error("invalid package declaration scope"));
                    }
                }
                Ok(Some(Arc::new(Self {
                    owners,
                    names: None,
                })))
            }
            _ => Err(input.error("unknown package declaration scope")),
        })
    }
}

impl DeclarationSelectionAuthority for PackageDeclarationScope {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        self.allows_declaration_selection(requester, owner)
    }
    fn package_label(&self, identity: PackageKeyIdentity) -> String {
        use std::fmt::Write;
        let mut digest = String::with_capacity(64);
        for byte in identity.digest() {
            let _ = write!(digest, "{byte:02x}");
        }
        match &self.names {
            Some(names) => match names.get(&identity) {
                Some(name) => format!("`{name}` ({digest})"),
                None => digest,
            },
            None => format!("package {digest}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_grants_only_listed_packages_and_their_direct_dependencies() {
        let package = |byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap();
        let (a, b, c, outsider) = (package(1), package(2), package(3), package(4));
        let scope = PackageDeclarationScope::from_direct_dependencies(
            &[a, b, c],
            [(a, b), (a, b), (b, c), (a, outsider), (outsider, b)],
        );
        assert!(scope.allows_declaration_selection(a, a));
        assert!(scope.allows_declaration_selection(a, b));
        assert!(!scope.allows_declaration_selection(a, c));
        assert!(!scope.allows_declaration_selection(a, outsider));
        assert!(!scope.allows_declaration_selection(outsider, b));
        assert_eq!(scope.owners[&a], [a, b]);
    }
}
