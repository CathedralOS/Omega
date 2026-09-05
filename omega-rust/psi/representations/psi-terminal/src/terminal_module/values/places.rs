use psi_core::{PlaceId, StructuralPlaceKind};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSegment {
    Field(String),
    FixedIndex(u64),
}

/// Whether a scalar-store carrier path is within the currently executable
/// bounded projection grammar: record fields, optionally followed by one
/// literal fixed-array index. Indexed stores must retain a record-field owner.
pub fn is_bounded_structural_scalar_store_path(path: &[StructuralPathSegment]) -> bool {
    let first_index = path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        .unwrap_or(path.len());
    let index_count = path.len() - first_index;
    path[..first_index].iter().all(
        |segment| matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty()),
    ) && path[first_index..]
        .iter()
        .all(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
        && (index_count == 0 || (first_index > 0 && index_count == 1))
}

impl From<String> for StructuralPathSegment {
    fn from(identity: String) -> Self {
        Self::Field(identity)
    }
}

impl From<&str> for StructuralPathSegment {
    fn from(identity: &str) -> Self {
        Self::Field(identity.to_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralPlaceDeclaration {
    pub id: PlaceId,
    pub kind: StructuralPlaceKind,
}
