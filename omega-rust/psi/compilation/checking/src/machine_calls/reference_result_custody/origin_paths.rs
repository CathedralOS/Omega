//! Possible fixed-array backings are an interface family, never runtime address
//! evaluation. Declaration-derived ingress and stored-descriptor transfer use
//! this concrete roster; their callers still check type, access and lineage.
//! Captured returned provenance uses typed element families instead and does
//! not enumerate storage merely to describe a borrowed element's origin.

use facts::PlaceSegment;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

/// Preserve every projection and array dimension while enumerating unresolved
/// selections from literal fixed extents. The existing origin capacity bounds
/// the Cartesian product before allocation. A selector runs only at formation.
pub(super) fn fixed_backing_paths(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<Vec<Vec<PlaceSegment>>> {
    let mut paths = vec![Vec::new()];
    for (position, segment) in segments.iter().enumerate() {
        if matches!(segment, PlaceSegment::Index { .. }) {
            let selected =
                crate::project_type_reference_from_segments(program, root, &segments[..position])?;
            let selected =
                crate::reference_shell(program, selected).map_or(selected, |shell| shell.referee);
            let selected = crate::peel_declared_constraints(program, selected);
            let TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Literal(length),
                ..
            } = program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            let length = *length;
            let count = paths.len().checked_mul(length)?;
            if count == 0 || count > super::MAX_REFERENCE_ORIGIN_LEAVES {
                return None;
            }
            let mut expanded = Vec::with_capacity(count);
            for path in paths {
                for index in 0..length {
                    let mut path = path.clone();
                    path.push(PlaceSegment::FixedIndex { index });
                    expanded.push(path);
                }
            }
            paths = expanded;
        } else {
            for path in &mut paths {
                path.push(*segment);
            }
        }
    }
    Some(paths)
}
