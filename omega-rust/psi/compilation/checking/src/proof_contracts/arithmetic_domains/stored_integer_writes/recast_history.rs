//! Follow evaluated representation views back to their physical interpretations.
//! Unsupported leaf translations stay unknown; the latest logical scalar type
//! cannot discharge a predecessor's narrower write obligation.
use super::*;

pub(super) fn projection(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    source: &crate::LocalWriteSource,
    suffix: &[PlaceSegment],
) -> Result<Option<(SymbolHandle, IntegerType)>, ()> {
    let (mut carrier, mut path) = source_coordinates(program, machine, state, source)?;
    path.extend_from_slice(suffix);
    let current = typed_storage_projection(program, carrier, &path)?;
    let logical_primitive = program.primitive_type_reference(
        places::unwrapped_type_reference(program, current.reference).ok_or(())?,
    );
    let mut stored = current.stored;
    let predecessors = source
        .recasts
        .iter()
        .map(|recast| {
            let (carrier, path) = predecessor_coordinates(program, machine, state, recast)?;
            let needs_fit = source_geometry::coordinate_requires_fit(program, carrier, &path)?;
            Ok((carrier, path, needs_fit))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    for (index, recast) in source.recasts.iter().enumerate().rev() {
        if !predecessors[..=index].iter().any(|(_, _, needed)| *needed) {
            break;
        }
        if carrier != recast.target_type {
            return Err(());
        }
        let (previous_carrier, mut previous_path, _) = predecessors[index].clone();
        let previous = typed_storage_projection(program, previous_carrier, &previous_path)?;
        let previous_type =
            places::unwrapped_type_reference(program, previous.reference).ok_or(())?;
        let target_type = places::unwrapped_type_reference(program, recast.target_type).ok_or(())?;
        let relative = if path.is_empty() {
            Vec::new()
        } else if program.normalized_type_identity(previous_type)
            == program.normalized_type_identity(target_type)
        {
            path.clone()
        } else if let [PlaceSegment::Field { symbol }] = path.as_slice() {
            crate::recast_record_source_leaf(program, previous_type, target_type, *symbol)
                .ok_or(())?
                .path
                .into_iter()
                .map(|segment| match segment {
                    crate::RecastLeafSegment::Field(symbol) => Ok(PlaceSegment::Field { symbol }),
                    crate::RecastLeafSegment::Element(index) => {
                        Ok(PlaceSegment::FixedIndex { index })
                    }
                })
                .collect::<Result<Vec<_>, ()>>()?
        } else {
            return Err(());
        };
        previous_path.extend(relative);
        let selected = typed_storage_projection(program, previous_carrier, &previous_path)?;
        if let Some(candidate) = selected.stored {
            // The current scalar-value checker works in the logical carrier.
            // A bit reinterpretation needs an explicit value transform before
            // it can establish this older carrier's encoded bounds.
            let primitive = program.primitive_type_reference(
                places::unwrapped_type_reference(program, selected.reference).ok_or(())?,
            );
            if primitive.is_none() || primitive != logical_primitive {
                return Err(());
            }
            if stored.is_some_and(|(_, storage)| storage != candidate.1) {
                return Err(());
            }
            stored = Some(candidate);
        }
        carrier = previous_carrier;
        path = previous_path;
    }
    Ok(stored)
}

pub(super) fn predecessor_coordinates(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    recast: &crate::RecastSource,
) -> Result<(TypeReferenceHandle, Vec<PlaceSegment>), ()> {
    let declared = root_type(program, machine, state, recast.root).ok_or(())?;
    let (carrier, path) = recast
        .representation
        .as_ref()
        .map(|(carrier, path)| (*carrier, path.clone()))
        .unwrap_or_else(|| (declared, recast.segments.clone()));
    program
        .type_reference_table
        .contains_type_reference(carrier)
        .then_some((carrier, path))
        .ok_or(())
}
