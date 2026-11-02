//! Exact descriptor counts from completed pointer geometry at the consumer's
//! storage cut. Every alternative must select the same count. An enclosure,
//! lifetime, stale initializer or unresolved view cannot supply an extent.

use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandFact, FlowStorageSnapshot};
use facts::PlaceSegment;

pub fn descriptor_extent(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    current: Handle<FlowStorageSnapshot>,
) -> Option<u64> {
    let mut pending = vec![(value, path.to_vec())];
    let mut visited = Vec::new();
    let mut extent = None;
    while let Some((value, path)) = pending.pop() {
        if visited.contains(&(value, path.clone())) {
            return None;
        }
        visited.push((value, path.clone()));
        let selected = super::reference_captures_at_path(program, control, value, &path, current)?
            .captures()?;
        if selected.is_empty() {
            return None;
        }
        for selected in selected {
            if !selected.projection.is_empty() {
                pending.push((selected.capture, selected.projection));
                continue;
            }
            let capture = control.operands.get(selected.capture);
            let element =
                crate::collection_element_type_reference(program, capture.type_reference)?;
            let referents = control.operand_referents.span(capture.referents)?;
            if referents.is_empty() {
                return None;
            }
            for referent in referents {
                let segments = control.operand_referent_segments.span(referent.segments)?;
                let (prefix, range) = match segments.split_last() {
                    Some((PlaceSegment::FixedRange { start, end }, prefix)) => {
                        (prefix, Some((*start, *end)))
                    }
                    _ => (segments, None),
                };
                let source = crate::canonical_place_type_reference(
                    program,
                    capture.state_symbol,
                    capture.statement_index,
                    &crate::CanonicalPlace {
                        root: referent.root,
                        segments: prefix.to_vec(),
                    },
                )?;
                let source_element = crate::collection_element_type_reference(program, source)?;
                // Region coordinates count source elements. A recast cannot
                // borrow that count when its destination elements differ.
                if program.normalized_type_identity(element)
                    != program.normalized_type_identity(source_element)
                {
                    return None;
                }
                let count = match range {
                    Some((start, end)) => {
                        let end = if end == usize::MAX {
                            crate::fixed_array_type_length(program, source)?
                        } else {
                            end
                        };
                        u64::try_from(end.checked_sub(start)?).ok()?
                    }
                    None => u64::try_from(crate::fixed_array_type_length(program, source)?).ok()?,
                };
                if extent.is_some_and(|prior| prior != count) {
                    return None;
                }
                extent = Some(count);
            }
        }
    }
    extent
}
