//! Reachable physical widths for calls whose write effects are not yet known.
//! A whole aggregate can contain encoded leaves even when its own projection is
//! not an IntegerAt field. Stored references need their captured referents;
//! declaration traversal alone cannot establish those contents' encodings.
use super::*;

pub fn stored_integer_source_may_require_fit(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<bool, ()> {
    let targets =
        crate::layout_checking_targets(program, source_layout_roots(program, state, source)?)?;
    let mut requires_fit = false;
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        requires_fit |= source_may_require_fit_alternative(&alternative, state, source)?;
    }
    Ok(requires_fit)
}

fn source_may_require_fit_alternative(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<bool, ()> {
    let (carrier, path) = source_coordinates(program, state, source)?;
    if coordinate_requires_fit(program, carrier, &path)? {
        return Ok(true);
    }
    for recast in &source.recasts {
        let (carrier, path) = recast_history::predecessor_coordinates(program, state, recast)?;
        if coordinate_requires_fit(program, carrier, &path)? {
            let previous = typed_storage_projection(program, carrier, &path)?;
            // A canonical physical view writes the same valid bytes, not a
            // value in its predecessor's wider logical carrier. Keep historical
            // fitting requirements unless the exact representation relation
            // establishes that distinction; merely having a recast is not proof.
            if !crate::value_custody::recasts::recast_representations_equivalent(
                program,
                previous.reference,
                recast.target_type,
                previous.stored.map(|(_, storage)| storage),
            ) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn coordinate_requires_fit(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Result<bool, ()> {
    let projection = typed_storage_projection(program, carrier, path)?;
    if let Some((field, _)) = projection.stored {
        return field_requires_fit(program, field);
    }
    if !path.is_empty() && crate::reference_shell(program, projection.reference).is_some() {
        return Err(());
    }
    let reference = places::unwrapped_type_reference(program, projection.reference).ok_or(())?;
    descendants_require_fit(program, reference, &mut Vec::new())
}

fn field_requires_fit(program: &dyn crate::CheckingInput, field: SymbolHandle) -> Result<bool, ()> {
    for plan in &program.plan_laid_layouts {
        if let Some(index) = plan
            .field_symbols
            .iter()
            .position(|candidate| *candidate == field)
        {
            let geometry = plan
                .geometry_for_target(program.typed_program(), program.layout_target())
                .ok_or(())?;
            if let Some(integer) = geometry
                .integer_fields
                .iter()
                .find(|integer| integer.field_index == index)
            {
                return Ok(!integer.write_is_total);
            }
        }
    }
    Err(())
}

fn descendants_require_fit(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> Result<bool, ()> {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
        || crate::reference_shell(program, reference).is_some()
    {
        return Err(());
    }
    let reference = places::unwrapped_type_reference(program, reference).ok_or(())?;
    if crate::proof_only_classification(program)
        .proof_only_mention(program, reference)
        .is_some()
    {
        return Ok(false);
    }
    // This is a may-reach union over declared owned edges. A backedge adds
    // no new fields; siblings on the active walk are still inspected.
    if active.contains(&reference) {
        return Ok(false);
    }
    if program.primitive_type_reference(reference).is_some() {
        return Ok(false);
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Unit => return Ok(false),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            active.push(reference);
            let result = descendants_require_fit(program, *element_type, active);
            active.pop();
            return result;
        }
        _ => {}
    }
    let data = places::data_definition_for_type(program, reference).ok_or(())?;
    active.push(reference);
    let result = (|| {
        for field in program.data_member_fields(program.data_members(data)) {
            if field.relevance.is_erased() {
                continue;
            }
            if stored_integer_field_type(program, reference, field.symbol)?.is_some()
                && field_requires_fit(program, field.symbol)?
            {
                return Ok(true);
            }
            if descendants_require_fit(program, field.type_reference, active)? {
                return Ok(true);
            }
        }
        Ok(false)
    })();
    active.pop();
    result
}
