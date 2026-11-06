//! Borrow canonical declarations for queries that need either keyed lookup or
//! a complete geometry walk. No query builds another copy of the catalog.
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::StructuralTypeDeclaration;

/// Queries preserve canonical ID order. Unit validation rejects unordered or
/// duplicate declarations before a slice reaches ownership replay.
pub(crate) trait StructuralTypes {
    fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration>;
    fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone;
}

// Unit catalog validation establishes strict ID order before function replay.
impl StructuralTypes for [StructuralTypeDeclaration] {
    fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self[position])
    }

    fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone {
        self.iter()
    }
}
