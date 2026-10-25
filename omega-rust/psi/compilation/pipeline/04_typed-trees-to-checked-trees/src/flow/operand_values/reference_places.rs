//! Stored descriptors come from their actual completed value graph. A match
//! contributes every feasible predecessor; a constructor with a provably
//! different enclosing case contributes no address. This query does not prove
//! that a payload may be accessed: the ordinary live-case check still does so.
//! Unknown producers and unknown saved descriptor inventories decline without
//! substituting a declaration initializer or a newer source descriptor.

use super::super::CapturedReferencePlace;
use super::finite_projection_paths;
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandCompletion, FlowOperandFact};
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;
use validation::CanonicalPlace;

pub(crate) fn captured_reference_places_with_formation(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    capture: Handle<FlowOperandFact>,
    reference: TypeReferenceHandle,
    projection: &[PlaceSegment],
) -> Option<Vec<CapturedReferencePlace>> {
    let mut places = Vec::new();
    collect_reference_places(
        program,
        control,
        capture,
        reference,
        projection,
        &mut Vec::new(),
        &mut places,
    )?;
    Some(places)
}

fn collect_reference_places(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    capture: Handle<FlowOperandFact>,
    reference: TypeReferenceHandle,
    projection: &[PlaceSegment],
    ancestors: &mut Vec<Handle<FlowOperandFact>>,
    places: &mut Vec<CapturedReferencePlace>,
) -> Option<()> {
    if !control.operands.is_valid(capture) || ancestors.contains(&capture) {
        return None;
    }
    let operand = control.operands.get(capture);
    ancestors.push(capture);
    let result = collect_operand_reference_places(
        program, control, operand, reference, projection, ancestors, places,
    );
    ancestors.pop();
    result
}

pub(crate) fn captured_operand_reference_places_with_formation(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    reference: TypeReferenceHandle,
    projection: &[PlaceSegment],
) -> Option<Vec<CapturedReferencePlace>> {
    let mut places = Vec::new();
    collect_operand_reference_places(
        program,
        control,
        operand,
        reference,
        projection,
        &mut Vec::new(),
        &mut places,
    )?;
    Some(places)
}

fn collect_operand_reference_places(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    reference: TypeReferenceHandle,
    projection: &[PlaceSegment],
    ancestors: &mut Vec<Handle<FlowOperandFact>>,
    places: &mut Vec<CapturedReferencePlace>,
) -> Option<()> {
    if projection
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
    {
        for selected in finite_projection_paths(program, reference, projection)? {
            collect_operand_reference_places(
                program, control, operand, reference, &selected, ancestors, places,
            )?;
        }
        return Some(());
    }
    if super::super::literal_value_path_is_inactive(
        program,
        operand.expression,
        reference,
        projection,
    ) {
        return Some(());
    }
    if let FlowOperandCompletion::Selected(alternatives) = operand.completion {
        let alternatives = control.operand_alternatives.span(alternatives)?;
        if alternatives.is_empty() {
            return None;
        }
        for alternative in alternatives {
            if !alternative.source_arm.is_valid() {
                return None;
            }
            collect_reference_places(
                program,
                control,
                alternative.value,
                reference,
                projection,
                ancestors,
                places,
            )?;
        }
        return Some(());
    }
    if let FlowOperandCompletion::Copied { copy, path } = operand.completion {
        if !control.copied_values.is_valid(copy) {
            return None;
        }
        let copied = control.copied_values.get(copy);
        let mut projection_from_copy = control.operand_child_paths.span(path)?.to_vec();
        projection_from_copy.extend_from_slice(projection);
        return collect_leaf_places(control, copied.references, &projection_from_copy, places);
    }
    if !operand.reference_leaves.is_empty() {
        return collect_leaf_places(control, operand.reference_leaves, projection, places);
    }
    if super::super::operand_contexts::operand_binds_live_view(program, reference) {
        let referents = control.operand_referents.span(operand.referents)?;
        if referents.is_empty() {
            return None;
        }
        for referent in referents {
            let place =
                CapturedReferencePlace::from_referent(control, referent)?.projected(projection)?;
            if !places.contains(&place) {
                places.push(place);
            }
        }
        return Some(());
    }
    let (child, child_reference, consumed) =
        super::captured_operand_child_at_path(program, control, operand, reference, projection)?;
    collect_reference_places(
        program,
        control,
        child,
        child_reference,
        &projection[consumed..],
        ancestors,
        places,
    )
}

fn collect_leaf_places(
    control: &FlowControlFacts,
    references: arena::HandleSpan<checked_trees::FlowCapturedReferenceLeaf>,
    projection: &[PlaceSegment],
    places: &mut Vec<CapturedReferencePlace>,
) -> Option<()> {
    let mut selected = None;
    for leaf in control.captured_reference_leaves.span(references)? {
        let path = control.operand_child_paths.span(leaf.path)?;
        if projection.starts_with(path) && selected.replace((leaf, path)).is_some() {
            return None;
        }
    }
    let (leaf, path) = selected?;
    let suffix = &projection[path.len()..];
    let referents = control.operand_referents.span(leaf.referents)?;
    if referents.is_empty() {
        return None;
    }
    for referent in referents {
        let place = CapturedReferencePlace::from_referent(control, referent)?.projected(suffix)?;
        if !places.contains(&place) {
            places.push(place);
        }
    }
    Some(())
}

/// Effect and loan geometry consumes no historical case authority. This one-way
/// projection must not be used to rebuild completed descriptor value rows.
pub(crate) fn captured_reference_places(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    capture: Handle<FlowOperandFact>,
    reference: TypeReferenceHandle,
    projection: &[PlaceSegment],
) -> Option<Vec<CanonicalPlace>> {
    Some(
        captured_reference_places_with_formation(program, control, capture, reference, projection)?
            .into_iter()
            .map(|reference| reference.place)
            .collect(),
    )
}
