//! Shared module declarations; function views retain this catalog, not copies.
//! Reference targets without demanded storage remain exact identity rows,
//! disjoint from concrete definitions. Every later IR retains both rosters;
//! only actual owned storage or projection asks for a concrete declaration.

use std::ops::Deref;
use std::sync::Arc;

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralReferentIdentity, StructuralTypeDeclaration};

/// Immutable shared structural declarations at native compilation boundaries.
///
/// Cloning retains the same storage. Builders and corruption tests must request
/// copy-on-write explicitly; editing one revision cannot change another. Equality
/// remains content-based, with a shared-allocation fast path, and is not evidence
/// of semantic admission.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralTypeCatalog(
    Arc<Vec<StructuralTypeDeclaration>>,
    Arc<Vec<StructuralReferentIdentity>>,
    Option<Arc<RetainedQuotients>>,
);

#[derive(Clone, Debug, PartialEq, Eq)]
struct RetainedQuotients {
    catalog: terminal_psi::TerminalQuotientCatalog,
    canonical: Vec<u8>,
}
static EMPTY_QUOTIENTS: terminal_psi::TerminalQuotientCatalog =
    terminal_psi::TerminalQuotientCatalog {
        representations: Vec::new(),
        operations: Vec::new(),
        adaptations: Vec::new(),
    };
impl StructuralTypeCatalog {
    /// Retain exact immutable metadata and its canonical encoding together.
    /// This checks serialization, not theorem admission.
    pub fn with_quotients(
        mut self,
        catalog: terminal_psi::TerminalQuotientCatalog,
    ) -> Result<Self, terminal_codec::CodecError> {
        self.2 = if catalog == EMPTY_QUOTIENTS {
            None
        } else {
            let canonical = terminal_codec::encode_quotient_catalog(&catalog)?;
            Some(Arc::new(RetainedQuotients { catalog, canonical }))
        };
        Ok(self)
    }
    pub fn quotients(&self) -> &terminal_psi::TerminalQuotientCatalog {
        self.2.as_ref().map_or(&EMPTY_QUOTIENTS, |row| &row.catalog)
    }
    pub fn quotient_identity_bytes(&self) -> &[u8] {
        self.2.as_ref().map_or(&[], |row| row.canonical.as_slice())
    }

    /// Retain storage definitions and identity-only referents in one shared
    /// namespace. Neither copying a pointer nor naming its target creates a
    /// storage definition; projections still require the concrete declaration.
    pub fn new(
        declarations: Vec<StructuralTypeDeclaration>,
        referents: Vec<StructuralReferentIdentity>,
    ) -> Self {
        Self(Arc::new(declarations), Arc::new(referents), None)
    }

    pub fn referents(&self) -> &[StructuralReferentIdentity] {
        self.1.as_slice()
    }

    pub fn as_slice(&self) -> &[StructuralTypeDeclaration] {
        self.0.as_slice()
    }

    /// Start an independently mutable revision, copying only when shared.
    pub fn make_mut(&mut self) -> &mut Vec<StructuralTypeDeclaration> {
        Arc::make_mut(&mut self.0)
    }
}

impl From<Vec<StructuralTypeDeclaration>> for StructuralTypeCatalog {
    fn from(declarations: Vec<StructuralTypeDeclaration>) -> Self {
        Self::new(declarations, Vec::new())
    }
}

impl FromIterator<StructuralTypeDeclaration> for StructuralTypeCatalog {
    fn from_iter<T: IntoIterator<Item = StructuralTypeDeclaration>>(declarations: T) -> Self {
        Self::from(declarations.into_iter().collect::<Vec<_>>())
    }
}

impl Deref for StructuralTypeCatalog {
    type Target = [StructuralTypeDeclaration];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a> IntoIterator for &'a StructuralTypeCatalog {
    type Item = &'a StructuralTypeDeclaration;
    type IntoIter = std::slice::Iter<'a, StructuralTypeDeclaration>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl PartialEq<Vec<StructuralTypeDeclaration>> for StructuralTypeCatalog {
    fn eq(&self, other: &Vec<StructuralTypeDeclaration>) -> bool {
        self.2.is_none() && self.referents().is_empty() && self.as_slice() == other.as_slice()
    }
}

impl PartialEq<StructuralTypeCatalog> for Vec<StructuralTypeDeclaration> {
    fn eq(&self, other: &StructuralTypeCatalog) -> bool {
        other.2.is_none() && other.referents().is_empty() && self.as_slice() == other.as_slice()
    }
}

/// Return the sole borrowed identity match, stopping at the second match.
pub(super) fn unique_structural_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    identity: StructuralTypeId,
) -> Option<&'a StructuralTypeDeclaration> {
    let mut declarations = types.filter(|declaration| declaration.id == identity);
    match (declarations.next(), declarations.next()) {
        (Some(declaration), None) => Some(declaration),
        _ => None,
    }
}
