//! Reachable physical widths for calls whose write effects are not yet known.
//! A whole aggregate can contain encoded leaves even when its own projection is
//! not an IntegerAt field. Stored references need their captured referents;
//! declaration traversal alone cannot establish those contents' encodings.
use super::*;

/// Physical demands and symbolic leaves are independent: an open member must
/// not hide a concrete narrowed sibling. Only the invocation owner can decide
/// whether a symbolic leaf is a template or an incomplete concrete application.
#[derive(Clone, Copy, Default)]
pub struct StoredIntegerReach {
    pub requires_fit: bool,
    pub has_open_parameters: bool,
}

impl StoredIntegerReach {
    fn join(&mut self, other: Self) {
        self.requires_fit |= other.requires_fit;
        self.has_open_parameters |= other.has_open_parameters;
    }

    fn concrete(self) -> Result<bool, ()> {
        if self.has_open_parameters {
            Err(())
        } else {
            Ok(self.requires_fit)
        }
    }
}

pub(super) fn stored_integer_source_may_require_fit(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<bool, ()> {
    stored_integer_source_reach(program, state, source)?.concrete()
}

pub fn stored_integer_source_reach(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<StoredIntegerReach, ()> {
    let layout_context = crate::LayoutCheckingContext::for_owner(program, state.symbol);
    let program: &dyn crate::CheckingInput = &layout_context;
    let targets =
        crate::layout_checking_targets(program, source_layout_roots(program, state, source)?)?;
    let mut reach = StoredIntegerReach::default();
    for target in targets {
        let alternative = crate::LayoutCheckingContext::new(program, target);
        reach.join(source_reach_alternative(&alternative, state, source)?);
    }
    Ok(reach)
}

fn source_reach_alternative(
    program: &dyn crate::CheckingInput,
    state: &State,
    source: &crate::LocalWriteSource,
) -> Result<StoredIntegerReach, ()> {
    let (carrier, path) = source_coordinates(program, state, source)?;
    let mut reach = coordinate_reach(program, carrier, &path)?;
    for recast in &source.recasts {
        let (carrier, path) = recast_history::predecessor_coordinates(program, state, recast)?;
        let previous_reach = coordinate_reach(program, carrier, &path)?;
        reach.has_open_parameters |= previous_reach.has_open_parameters;
        if previous_reach.requires_fit {
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
                reach.requires_fit = true;
            }
        }
    }
    Ok(reach)
}

pub(super) fn coordinate_requires_fit(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Result<bool, ()> {
    coordinate_reach(program, carrier, path)?.concrete()
}

fn coordinate_reach(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Result<StoredIntegerReach, ()> {
    let projection = typed_storage_projection(program, carrier, path)?;
    if let Some((field, _)) = projection.stored {
        return Ok(StoredIntegerReach {
            requires_fit: field_requires_fit(program, field)?,
            has_open_parameters: false,
        });
    }
    if !path.is_empty() && crate::reference_shell(program, projection.reference).is_some() {
        return Err(());
    }
    let reference = places::unwrapped_type_reference(program, projection.reference).ok_or(())?;
    descendants_reach(program, reference, &mut Vec::new())
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

fn descendants_reach(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    active: &mut Vec<TypeReferenceHandle>,
) -> Result<StoredIntegerReach, ()> {
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
        return Ok(StoredIntegerReach::default());
    }
    // This is a may-reach union over declared owned edges. A backedge adds
    // no new fields; siblings on the active walk are still inspected.
    if active.contains(&reference) {
        return Ok(StoredIntegerReach::default());
    }
    if program.primitive_type_reference(reference).is_some() {
        return Ok(StoredIntegerReach::default());
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Unit => return Ok(StoredIntegerReach::default()),
        TypeReferenceNode::Named { symbol, .. }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter =>
        {
            return Ok(StoredIntegerReach {
                requires_fit: false,
                has_open_parameters: true,
            });
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            active.push(reference);
            let result = descendants_reach(program, *element_type, active);
            active.pop();
            return result;
        }
        _ => {}
    }
    let data = places::data_definition_for_type(program, reference).ok_or(())?;
    active.push(reference);
    let result = (|| {
        let mut reach = StoredIntegerReach::default();
        for field in program.data_member_fields(program.data_members(data)) {
            if field.relevance.is_erased() {
                continue;
            }
            if stored_integer_field_type(program, reference, field.symbol)?.is_some()
                && field_requires_fit(program, field.symbol)?
            {
                reach.requires_fit = true;
            }
            reach.join(descendants_reach(program, field.type_reference, active)?);
        }
        Ok(reach)
    })();
    active.pop();
    result
}
