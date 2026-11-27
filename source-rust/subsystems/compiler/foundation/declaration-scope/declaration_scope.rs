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

    /// Whether source checked under `previous` may be reused under `current`.
    ///
    /// Each requester must keep every package it was allowed to select.
    /// A grant only decides whether an already resolved selection is
    /// permitted; it never changes what a name resolves to. A retained check
    /// that passed with fewer grants therefore still passes with more.
    ///
    /// No scope means a package selects only its own and toolchain
    /// declarations, the smallest grant any current scope can give it.
    /// Losing a grant, or losing the scope, requires a new check.
    pub fn require_current(
        previous: Option<&Self>,
        current: Option<&Self>,
        requesters: impl IntoIterator<Item = PackageKeyIdentity>,
    ) -> Result<(), &'static str> {
        let Some(current) = current else {
            return match previous {
                None => Ok(()),
                Some(_) => Err("current source has lost its declaration selection authority"),
            };
        };
        for requester in requesters {
            let new = current
                .owners
                .get(&requester)
                .ok_or("current declaration scope does not cover a requesting package")?;
            let kept = match previous {
                None => new.binary_search(&requester).is_ok(),
                Some(previous) => previous
                    .owners
                    .get(&requester)
                    .ok_or("retained declaration scope does not cover a requesting package")?
                    .iter()
                    .all(|owner| new.binary_search(owner).is_ok()),
            };
            if !kept {
                return Err("requesting package lost a declaration selection grant");
            }
        }
        Ok(())
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

    #[test]
    fn retained_source_survives_a_new_grant_but_not_a_lost_one() {
        let package = |byte| PackageKeyIdentity::from_digest([byte; 32]).unwrap();
        let (root, kit, other) = (package(1), package(2), package(3));
        let scope = |edges: &[(PackageKeyIdentity, PackageKeyIdentity)]| {
            PackageDeclarationScope::from_direct_dependencies(
                &[root, kit, other],
                edges.iter().copied(),
            )
        };
        let reuse = |previous: Option<&PackageDeclarationScope>,
                     current: Option<&PackageDeclarationScope>| {
            PackageDeclarationScope::require_current(previous, current, [root]).is_ok()
        };
        let none = scope(&[]);
        let with_kit = scope(&[(root, kit)]);
        let with_both = scope(&[(root, kit), (root, other)]);
        let only_other = scope(&[(root, other)]);

        // A Build checked before any dependency existed, and one checked
        // before `kit` was prepared, both keep their prefix once it is.
        assert!(reuse(None, None));
        assert!(reuse(None, Some(&with_kit)));
        assert!(reuse(Some(&none), Some(&with_kit)));
        assert!(reuse(Some(&with_kit), Some(&with_both)));
        assert!(reuse(Some(&with_kit), Some(&with_kit)));

        // Losing `kit`, or every grant, requires a new check.
        assert!(!reuse(Some(&with_kit), Some(&only_other)));
        assert!(!reuse(Some(&with_kit), Some(&none)));
        assert!(!reuse(Some(&with_kit), None));
        // A requester the current scope does not know is never reused.
        let unrelated = PackageDeclarationScope::from_direct_dependencies(&[kit], []);
        assert!(!reuse(None, Some(&unrelated)));
        assert!(!reuse(Some(&with_kit), Some(&unrelated)));
    }
}
