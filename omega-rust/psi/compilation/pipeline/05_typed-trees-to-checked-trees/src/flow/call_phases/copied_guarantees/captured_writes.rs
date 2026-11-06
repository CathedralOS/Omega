//! A saved pointer still names the captured element after its selector changes.
//! Use that storage to limit the call's writes. An unavailable address or a
//! different alias namespace keeps the original conservative footprint.

use super::ArgumentSources;
use crate::flow::mutation::{CallMutationWrite, StorageRegion};
use facts::PlaceSegment;
use typed_trees::signature::StateParameter;
use validation::CanonicalPlace;

/// A stored pointer crosses to another home. Only a write that stays inside
/// this formal's inline referent can use the captured actual as its enclosure.
pub(in crate::flow::call_phases) fn confined_write_parameter(
    program: &dyn validation::CheckingInput,
    parameters: &[StateParameter],
    source: &CanonicalPlace,
) -> Option<usize> {
    let position = parameters
        .iter()
        .position(|parameter| source.root.as_symbol() == Some(parameter.symbol))?;
    let mut reference =
        validation::reference_shell(program, parameters[position].type_reference)?.referee;
    for segment in &source.segments {
        if validation::reference_shell(program, reference).is_some() {
            return None;
        }
        reference = validation::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        )?;
    }
    validation::reference_shell(program, reference)
        .is_none()
        .then_some(position)
}

impl ArgumentSources<'_, '_> {
    pub(in crate::flow) fn captured_write_enclosures(
        &self,
        write: &CallMutationWrite,
    ) -> Option<Vec<(CanonicalPlace, CanonicalPlace)>> {
        let StorageRegion::Declared(written) = &write.region else {
            return None;
        };
        let Some(StorageRegion::Declared(source)) = &write.source else {
            return None;
        };
        let position = confined_write_parameter(self.program, self.parameters, source)?;
        let actuals = self.referent_places(position)?;
        if actuals.is_empty()
            || !actuals.iter().all(|actual| {
                actual.root.is_valid()
                    && actual.root == written.root
                    && actual
                        .segments
                        .iter()
                        .all(crate::flow::place_segment_has_static_identity)
            })
        {
            return None;
        }
        actuals
            .iter()
            .map(|actual| {
                Some((
                    intersect_captured_enclosure(written, actual)?,
                    actual.clone(),
                ))
            })
            .collect()
    }
}

/// Intersect existing coordinates in one storage namespace. This neither
/// follows a pointer nor appends a logical view's fields to its backing.
/// An unrepresentable intersection withholds refinement instead of an effect.
pub(in crate::flow::call_phases) fn intersect_captured_enclosure(
    written: &CanonicalPlace,
    captured: &CanonicalPlace,
) -> Option<CanonicalPlace> {
    if written.root != captured.root {
        return None;
    }
    let mut segments = Vec::with_capacity(written.segments.len().max(captured.segments.len()));
    for (&written, &captured) in written.segments.iter().zip(&captured.segments) {
        let segment = if written == captured {
            written
        } else {
            match (written, captured) {
                (
                    PlaceSegment::Index { .. } | PlaceSegment::OpaqueRange,
                    captured @ (PlaceSegment::FixedIndex { .. } | PlaceSegment::FixedRange { .. }),
                ) => captured,
                (PlaceSegment::FixedIndex { index }, PlaceSegment::FixedRange { start, end })
                | (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index })
                    if start <= index && index < end =>
                {
                    PlaceSegment::FixedIndex { index }
                }
                (
                    PlaceSegment::FixedRange {
                        start: left_start,
                        end: left_end,
                    },
                    PlaceSegment::FixedRange {
                        start: right_start,
                        end: right_end,
                    },
                ) => {
                    let start = left_start.max(right_start);
                    let end = left_end.min(right_end);
                    if start >= end {
                        return None;
                    }
                    PlaceSegment::FixedRange { start, end }
                }
                _ => return None,
            }
        };
        segments.push(segment);
    }
    let common = written.segments.len().min(captured.segments.len());
    segments.extend_from_slice(&written.segments[common..]);
    segments.extend_from_slice(&captured.segments[common..]);
    Some(CanonicalPlace {
        root: written.root,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(segments: Vec<PlaceSegment>) -> CanonicalPlace {
        CanonicalPlace {
            root: facts::PlaceRoot::Unknown,
            segments,
        }
    }

    #[test]
    fn captured_element_keeps_the_original_descendant_write() {
        // The erased selector has no current value to consult. Its completed
        // pointer supplies element 1 while the original write still selects
        // child 3 inside that element.
        let written = place(vec![
            PlaceSegment::Index {
                expression: typed_trees::expression::ExpressionHandle::invalid(),
            },
            PlaceSegment::FixedIndex { index: 3 },
        ]);
        let captured = place(vec![PlaceSegment::FixedIndex { index: 1 }]);
        assert_eq!(
            intersect_captured_enclosure(&written, &captured),
            Some(place(vec![
                PlaceSegment::FixedIndex { index: 1 },
                PlaceSegment::FixedIndex { index: 3 },
            ])),
        );
    }

    #[test]
    fn captured_window_intersects_existing_write_bounds() {
        let written = place(vec![PlaceSegment::FixedRange { start: 0, end: 8 }]);
        let captured = place(vec![PlaceSegment::FixedRange { start: 3, end: 5 }]);
        assert_eq!(
            intersect_captured_enclosure(&written, &captured),
            Some(captured.clone())
        );
        let written = place(vec![PlaceSegment::FixedIndex { index: 4 }]);
        assert_eq!(
            intersect_captured_enclosure(&written, &captured),
            Some(written)
        );
    }

    #[test]
    fn conflicting_or_unrelated_coordinates_withhold_refinement() {
        let written = place(vec![PlaceSegment::FixedIndex { index: 0 }]);
        let captured = place(vec![PlaceSegment::FixedIndex { index: 1 }]);
        assert_eq!(intersect_captured_enclosure(&written, &captured), None);
        let mut other_root = written.clone();
        other_root.root =
            facts::PlaceRoot::Expression(typed_trees::expression::ExpressionHandle::invalid());
        assert_eq!(intersect_captured_enclosure(&written, &other_root), None);
    }
}
