//! Publish write footprints in the original storage namespace.
//!
//! A recast keeps its logical field selectors for access and interpretation,
//! but those names cannot be appended to the backing's path. Scalar stores
//! translate through the captured geometry; writes without an exact leaf cover
//! the original backing. Geometry establishes overlap, never a numeric fact.

use super::{FramePathPrecision, FramePlaceOrigin, append_place_suffix, coordinate_storage_path};
use crate::LocalWriteSource;

pub(in crate::machine_calls::calls::write_frames) fn project_write_path(
    program: &dyn crate::CheckingInput,
    origin: &FramePlaceOrigin,
    suffix: &str,
) -> String {
    if origin.source.recasts.is_empty() {
        return match origin.precision {
            FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
            FramePathPrecision::CollectionCoarse
                if origin.source.builtin_coordinates
                    && origin.source.segments.iter().any(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::FixedIndex { .. }
                                | facts::PlaceSegment::Index { .. }
                                | facts::PlaceSegment::FixedRange { .. }
                                | facts::PlaceSegment::OpaqueRange
                        )
                    }) =>
            {
                if origin.path.ends_with("[*]") {
                    origin.path.clone()
                } else {
                    format!("{}[*]", origin.path)
                }
            }
            FramePathPrecision::CollectionCoarse => origin.path.clone(),
        };
    }
    let source = &origin.source;
    let exact = || {
        if origin.precision != FramePathPrecision::Exact || !source.builtin_coordinates {
            return None;
        }
        let (carrier, selectors) = source.representation.as_ref()?;
        let mut reference =
            crate::project_type_reference_from_segments(program, *carrier, selectors)?;
        let mut segments = Vec::new();
        if !suffix.is_empty() {
            for name in suffix.strip_prefix('.')?.split('.') {
                let definition =
                    crate::value_custody::places::data_definition_for_type(program, reference)?;
                let symbol = crate::value_custody::places::data_member_symbol_by_name(
                    program, definition, name,
                )?;
                crate::push_field_place_segments(program, &mut segments, symbol);
                reference = crate::project_type_reference_from_segments(
                    program,
                    reference,
                    &segments[segments.len() - 1..],
                )?;
            }
        }
        let projected = source.append_segments(&segments);
        let declaration = program.symbols.get(projected.root);
        let state = program.state_by_symbol(declaration.parent)?;
        LocalWriteSource::from(FramePlaceOrigin {
            path: origin.path.clone(),
            precision: origin.precision,
            source: projected,
        })
        .scalar_storage_coordinate(program, state)?
        .label(program)
    };
    let projected = exact();
    if let Some(path) = projected {
        return path;
    }
    // The first recast captured storage before any alternate interpretation.
    // A missing leaf is not purity: retain that entire backing footprint.
    let first = &source.recasts[0];
    if !first.root.is_valid() {
        // Path-only provenance cannot select a leaf beneath the new view.
        // Keep its established storage root, covering every stored referent
        // when the ordinary origin expansion visits that carrier.
        return super::split_place_root(&origin.path).0.to_owned();
    }
    coordinate_storage_path(program, program.symbols.name(first.root), &first.segments).0
}
