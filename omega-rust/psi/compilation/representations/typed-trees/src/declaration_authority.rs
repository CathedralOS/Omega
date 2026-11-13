//! Captured package visibility belongs to the retained typed source input.
//! Comparing its actual requester scopes authorizes no source proof or body.

use semantic_vocabulary::PackageKeyIdentity;
use semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority;

#[derive(Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum RecordedDeclarationAuthority {
    Absent,
    Packages(std::collections::BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>),
}
impl RecordedDeclarationAuthority {
    pub fn capture(
        authority: Option<&dyn DeclarationSelectionAuthority>,
    ) -> Result<Self, &'static str> {
        let Some(authority) = authority else {
            return Ok(Self::Absent);
        };
        let mut scope = authority
            .declaration_selection_scope()
            .ok_or("declaration selection predicate does not export a complete comparison scope")?;
        for owners in scope.values_mut() {
            owners.sort_unstable();
            owners.dedup();
        }
        Ok(Self::Packages(scope))
    }
    pub fn has_authority(&self) -> bool {
        matches!(self, Self::Packages(_))
    }
    pub fn allows(&self, requester: PackageKeyIdentity, owner: PackageKeyIdentity) -> Option<bool> {
        match self {
            Self::Absent => None,
            Self::Packages(scope) => scope.get(&requester).map(|owners| owners.contains(&owner)),
        }
    }
    pub fn require_current(
        &self,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        sources: &source::SourceMap,
    ) -> Result<(), &'static str> {
        self.require_requesters(
            authority,
            sources
                .selected_files()
                .filter(|file| !file.origin.is_toolchain())
                .filter_map(|file| file.package_identity),
        )
    }
    pub fn require_current_sites(
        &self,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        program: &crate::TypedTrees,
        sites: impl Iterator<Item = source::SourceSpan>,
    ) -> Result<(), &'static str> {
        let mut requesters = std::collections::BTreeSet::new();
        for site in sites {
            let file = program
                .symbols
                .source_file(site)
                .ok_or("retained pending invocation has no source custody")?;
            if !file.origin.is_toolchain()
                && let Some(requester) = file.package_identity
            {
                requesters.insert(requester);
            }
        }
        self.require_requesters(authority, requesters.into_iter())
    }
    fn require_requesters(
        &self,
        authority: Option<&dyn DeclarationSelectionAuthority>,
        requesters: impl Iterator<Item = PackageKeyIdentity>,
    ) -> Result<(), &'static str> {
        match (self, Self::capture(authority)?) {
            (Self::Absent, Self::Absent) => Ok(()),
            (Self::Packages(previous), Self::Packages(current)) => {
                for requester in requesters {
                    let old = previous
                        .get(&requester)
                        .ok_or("retained declaration scope does not cover a requesting package")?;
                    let new = current
                        .get(&requester)
                        .ok_or("current declaration scope does not cover a requesting package")?;
                    if old != new {
                        return Err("requesting package declaration scope changed");
                    }
                }
                Ok(())
            }
            _ => Err("presence of declaration selection authority changed"),
        }
    }
}
