//! Shared module declarations; function views retain this catalog, not copies.

use std::ops::Deref;
use std::sync::Arc;

use terminal_psi::StructuralTypeDeclaration;

/// Immutable shared structural declarations at native compilation boundaries.
///
/// Cloning retains the same storage. Builders and corruption tests must request
/// copy-on-write explicitly; editing one revision cannot change another. Equality
/// remains content-based, with a shared-allocation fast path, and is not evidence
/// of semantic admission.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralTypeCatalog(Arc<Vec<StructuralTypeDeclaration>>);

impl StructuralTypeCatalog {
    pub fn as_slice(&self) -> &[StructuralTypeDeclaration] {
        self.0.as_slice()
    }

    /// Start an independently mutable revision, copying only when shared.
    pub fn make_mut(&mut self) -> &mut Vec<StructuralTypeDeclaration> {
        Arc::make_mut(&mut self.0)
    }

    /// Inspect allocation sharing; this is not a semantic validation operation.
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl From<Vec<StructuralTypeDeclaration>> for StructuralTypeCatalog {
    fn from(declarations: Vec<StructuralTypeDeclaration>) -> Self {
        Self(Arc::new(declarations))
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
        self.as_slice() == other.as_slice()
    }
}

impl PartialEq<StructuralTypeCatalog> for Vec<StructuralTypeDeclaration> {
    fn eq(&self, other: &StructuralTypeCatalog) -> bool {
        self.as_slice() == other.as_slice()
    }
}
