//! Project actual completed captures, preserving constructed-child epochs
//! and exact retained profiles. These associations supply no new authority.
use checked_trees::{FlowControlFacts, FlowOperandFact};
use facts::PlaceSegment;
use typed_trees::types::TypeReferenceHandle;

pub fn operand_child_type(
    program: &dyn crate::CheckingInput,
    parent: TypeReferenceHandle,
    destination: &[PlaceSegment],
    declared: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    parent
        .is_valid()
        .then(|| crate::project_type_reference_from_segments(program, parent, destination))
        .flatten()
        .or_else(|| declared.is_valid().then_some(declared))
}

pub fn captured_retention_profiles_at_path(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    path: &[PlaceSegment],
) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
    fn operand_profiles(
        program: &dyn crate::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        path: &[PlaceSegment],
        active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
        active_copies: &mut Vec<arena::Handle<checked_trees::FlowCopiedValue>>,
        active_selections: &mut Vec<arena::HandleSpan<checked_trees::FlowOperandAlternative>>,
    ) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
            if active_selections.contains(&alternatives) { return None; }
            let rows = control.operand_alternatives.span(alternatives)?;
            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid() || !control.operands.is_valid(row.value)) {
                return None;
            }
            active_selections.push(alternatives);
            let result = (|| {
                let mut profiles = Vec::new();
                for row in rows {
                    for profile in operand_profiles(
                        program, control, control.operands.get(row.value), path,
                        active, active_copies, active_selections,
                    )? {
                        if !profiles.contains(&profile) { profiles.push(profile); }
                    }
                }
                (!profiles.is_empty()).then_some(profiles)
            })();
            active_selections.pop();
            return result;
        }
        if !operand.reference_leaves.is_empty() {
            let mut matching = control
                .captured_reference_leaves
                .span_or_empty(operand.reference_leaves)
                .iter()
                .filter(|leaf| {
                    let selected = control.operand_child_paths.span_or_empty(leaf.path);
                    selected.len() == path.len()
                        && selected.iter().zip(path).all(|(left, right)| {
                            crate::storage_path_relations::canonical_place_segments_equal(program, *left, *right)
                        })
                });
            let leaf = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            return profile_at_path(program, control, leaf.retention, &[], active, active_copies, active_selections);
        }
        if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } =
            operand.completion
        {
            if !control.copied_values.is_valid(copy) || active_copies.contains(&copy) {
                return None;
            }
            active_copies.push(copy);
            let selected = (|| {
                let copy = control.copied_values.get(copy);
                let mut selected_path = control.operand_child_paths.span_or_empty(prefix).to_vec();
                selected_path.extend_from_slice(path);
                let mut leaves = control
                    .captured_reference_leaves
                    .span_or_empty(copy.references)
                    .iter()
                    .filter(|leaf| {
                        control.operand_child_paths.span_or_empty(leaf.path) == selected_path
                    });
                if let Some(leaf) = leaves.next() {
                    if leaves.next().is_some() {
                        return None;
                    }
                    return profile_at_path(
                        program,
                        control,
                        leaf.retention,
                        &[],
                        active,
                        active_copies,
            active_selections,
                    );
                }
                if !control.operands.is_valid(copy.source) {
                    return None;
                }
                operand_profiles(
                    program,
                    control,
                    control.operands.get(copy.source),
                    &selected_path,
                    active,
                    active_copies,
            active_selections,
                )
            })();
            active_copies.pop();
            return selected;
        }
        let mut selected_child = None;
        for child in control
            .operand_children
            .span_or_empty(operand.constructed_children())
        {
            let destination = control.operand_child_paths.span_or_empty(child.path);
            if destination.is_empty() {
                return None;
            }
            if destination.len() <= path.len()
                && destination.iter().zip(path).all(|(left, right)| {
                    crate::storage_path_relations::canonical_place_segments_equal(program, *left, *right)
                })
            {
                if selected_child.is_some() || !control.operands.is_valid(child.value) {
                    return None;
                }
                selected_child = Some((child.value, destination.len()));
            }
        }
        if let Some((child, length)) = selected_child {
            return operand_profiles(
                program,
                control,
                control.operands.get(child),
                &path[length..],
                active,
                active_copies,
            active_selections,
            );
        }
        profile_at_path(
            program,
            control,
            operand.retention,
            path,
            active,
            active_copies,
            active_selections,
        )
    }

    fn profile_at_path(
        program: &dyn crate::CheckingInput,
        control: &FlowControlFacts,
        retention: arena::Handle<checked_trees::FlowOperandRetention>,
        path: &[PlaceSegment],
        active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
        active_copies: &mut Vec<arena::Handle<checked_trees::FlowCopiedValue>>,
        active_selections: &mut Vec<arena::HandleSpan<checked_trees::FlowOperandAlternative>>,
    ) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
        if !control.operand_retentions.is_valid(retention) || active.contains(&retention) {
            return None;
        }
        let profile = control.operand_retentions.get(retention);
        if path.is_empty() && profile.stored_projection.is_empty() {
            return Some(vec![retention]);
        }
        active.push(retention);
        let selected = (|| {
            let mut selected = control
                .operand_retained_leaves
                .span_or_empty(profile.leaves)
                .iter()
                .filter(|leaf| {
                    let candidate = control.operand_retained_paths.span_or_empty(leaf.path);
                    !candidate.is_empty()
                        && candidate.len() <= path.len()
                        && candidate.iter().zip(path).all(|(left, right)| {
                            crate::storage_path_relations::canonical_place_segments_equal(program, *left, *right)
                        })
                });
            if let Some(leaf) = selected.next() {
                if selected.next().is_some() {
                    return None;
                }
                let length = leaf.path.len();
                return profile_at_path(
                    program,
                    control,
                    leaf.retention,
                    &path[length..],
                    active,
                    active_copies,
            active_selections,
                );
            }
            if !profile.alternatives.is_empty() {
                let mut profiles = Vec::new();
                for alternative in profile.alternatives.handles() {
                    for selected in
                        profile_at_path(program, control, alternative, path, active, active_copies, active_selections)?
                    {
                        if !profiles.contains(&selected) {
                            profiles.push(selected);
                        }
                    }
                }
                return (!profiles.is_empty()).then_some(profiles);
            }
            let stored = profile.stored_value;
            if !control.operands.is_valid(stored) {
                return None;
            }
            let prefix = control
                .operand_retained_paths
                .span_or_empty(profile.stored_projection);
            let mut selected = prefix.to_vec();
            selected.extend_from_slice(path);
            operand_profiles(
                program,
                control,
                control.operands.get(stored),
                &selected,
                active,
                active_copies,
            active_selections,
            )
        })();
        active.pop();
        selected
    }
    operand_profiles(
        program,
        control,
        operand,
        path,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

pub fn project_captured_view_place(
    mut place: crate::CanonicalPlace,
    path: &[PlaceSegment],
) -> Option<crate::CanonicalPlace> {
    place.segments = facts::project_captured_view_segments(&place.segments, path)?;
    Some(place)
}

/// Follow one captured assembly edge, retaining the unconsumed projection.
/// A child may itself be an opaque call result: its remaining fields belong
/// to that completed value, not to the containing constructor's namespace.
pub fn captured_operand_child_at_path(
    program: &dyn crate::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<(arena::Handle<FlowOperandFact>, TypeReferenceHandle, usize)> {
    let mut selected = None;
    for child in control.operand_children.span(operand.constructed_children())? {
        let destination = control.operand_child_paths.span(child.path)?;
        if destination.is_empty() {
            return None;
        }
        if destination.len() <= path.len()
            && destination.iter().zip(path).all(|(left, right)| {
                crate::storage_path_relations::canonical_place_segments_equal(
                    program, *left, *right,
                )
            })
        {
            if selected.is_some() || !control.operands.is_valid(child.value) {
                return None;
            }
            selected = Some((
                child.value,
                operand_child_type(program, reference, destination, child.type_reference)?,
                destination.len(),
            ));
        }
    }
    selected
}
