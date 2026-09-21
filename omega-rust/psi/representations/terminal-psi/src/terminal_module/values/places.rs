use semantic_vocabulary::{PlaceId, StructuralPlaceKind};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPathSegment {
    Field(String),
    FixedIndex(u64),
    /// Cross a reference carrier's borrowed boundary. This never grants owned
    /// access to the referent or includes it in the carrier's cleanup.
    Referent,
}

/// Whether a scalar-store carrier path is within the currently executable
/// bounded projection grammar: record fields, optionally followed by one
/// literal fixed-array index. A bare fixed-array root has no record-field
/// owner, so its carrier path is the literal element index alone. Anything
/// after the first index — a second index or a further field — is excluded
/// by the grammar itself, not by path resolution, and `Referent` crossings
/// are outside the store contract: borrowing through another borrow's
/// boundary is different custody, not a projection. Dynamic indexes are not
/// representable here and stay rejected upstream; resolution still requires
/// each literal index below its declared extent.
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
        && index_count <= 1
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

#[cfg(test)]
mod tests {
    use super::*;

    fn field(identity: &str) -> StructuralPathSegment {
        StructuralPathSegment::Field(identity.to_owned())
    }

    #[test]
    fn scalar_store_carrier_paths_are_fields_then_one_literal_index() {
        for path in [
            vec![],
            vec![field("record")],
            vec![field("outer"), field("inner")],
            vec![StructuralPathSegment::FixedIndex(0)],
            vec![field("items"), StructuralPathSegment::FixedIndex(2)],
        ] {
            assert!(is_bounded_structural_scalar_store_path(&path), "{path:?}");
        }
        for path in [
            // A second index and a field after the index are outside the
            // grammar even where a resolver could still walk them.
            vec![
                StructuralPathSegment::FixedIndex(0),
                StructuralPathSegment::FixedIndex(1),
            ],
            vec![StructuralPathSegment::FixedIndex(0), field("nested")],
            vec![
                field("items"),
                StructuralPathSegment::FixedIndex(0),
                field("nested"),
            ],
            // Borrowing through another borrow's boundary is different
            // custody, not a projection.
            vec![StructuralPathSegment::Referent],
            vec![field("record"), StructuralPathSegment::Referent],
            vec![StructuralPathSegment::Referent, field("record")],
            // An empty identity names no record field.
            vec![StructuralPathSegment::Field(String::new())],
        ] {
            assert!(!is_bounded_structural_scalar_store_path(&path), "{path:?}");
        }
    }
}
