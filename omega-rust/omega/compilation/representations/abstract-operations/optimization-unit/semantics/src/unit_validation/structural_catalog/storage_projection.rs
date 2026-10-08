//! Storage coordinates are not always standalone value endpoints. A byte window
//! keeps its enclosing backing type and exact interval; treating it as a fresh
//! array would lose custody or admit value copies that call admission forbids.
//! Formation, bounds evidence and access remain the owning operation's checks.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralStorageProjection {
    Endpoint(StructuralTypeId),
    ByteWindow {
        backing: StructuralTypeId,
        offset: u64,
        length: u64,
    },
}

impl StructuralStorageProjection {
    /// Rebinding preserves endpoint presentation, or both the backing and exact
    /// interval of a region. Neither equality nor presentation grants custody.
    pub fn matches<'types>(
        self,
        declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        expected: Self,
    ) -> bool {
        match (self, expected) {
            (Self::Endpoint(actual), Self::Endpoint(expected)) => {
                terminal_semantics::borrowed_referent_matches(declarations, actual, expected)
            }
            (
                Self::ByteWindow {
                    backing: actual,
                    offset,
                    length,
                },
                Self::ByteWindow {
                    backing: expected,
                    offset: expected_offset,
                    length: expected_length,
                },
            ) => {
                offset == expected_offset
                    && length == expected_length
                    && terminal_semantics::borrowed_referent_matches(declarations, actual, expected)
            }
            _ => false,
        }
    }
}

pub fn structural_storage_projection<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralStorageProjection> {
    if matches!(
        path.last(),
        Some(StructuralPathSegment::FixedByteRange { .. })
    ) {
        let window =
            terminal_semantics::fixed_byte_storage_window(declarations.clone(), root, path)?;
        Some(StructuralStorageProjection::ByteWindow {
            backing: super::paths::structural_projection_type(
                declarations,
                root,
                window.backing_path,
            )?,
            offset: window.offset,
            length: window.length,
        })
    } else {
        super::paths::structural_projection_type(declarations, root, path)
            .map(StructuralStorageProjection::Endpoint)
    }
}
