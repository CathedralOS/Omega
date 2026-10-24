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
    ) -> Option<Vec<arena::Handle<checked_trees::FlowOperandRetention>>> {
        let mut selected_child = None;
        for child in control.operand_children.span_or_empty(operand.children) {
            let destination = control.operand_child_paths.span_or_empty(child.path);
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
            );
        }
        profile_at_path(program, control, operand.retention, path, active)
    }

    fn profile_at_path(
        program: &dyn crate::CheckingInput,
        control: &FlowControlFacts,
        retention: arena::Handle<checked_trees::FlowOperandRetention>,
        path: &[PlaceSegment],
        active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
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
                            crate::storage_path_relations::canonical_place_segments_equal(
                                program, *left, *right,
                            )
                        })
                });
            if let Some(leaf) = selected.next() {
                if selected.next().is_some() {
                    return None;
                }
                let length = leaf.path.len();
                return profile_at_path(program, control, leaf.retention, &path[length..], active);
            }
            if !profile.alternatives.is_empty() {
                let mut profiles = Vec::new();
                for alternative in profile.alternatives.handles() {
                    for selected in profile_at_path(program, control, alternative, path, active)? {
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
            )
        })();
        active.pop();
        selected
    }
    operand_profiles(program, control, operand, path, &mut Vec::new())
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
    for child in control.operand_children.span(operand.children)? {
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
