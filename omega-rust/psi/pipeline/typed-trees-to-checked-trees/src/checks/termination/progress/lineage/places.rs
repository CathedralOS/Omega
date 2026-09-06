//! Finite storage partitions for parameter values, not recursive referent snapshots.

use super::super::ProgressSubject;
use typed_trees::{TypedTrees, data::DataMember, machine::Machine};

/// Select a finite owned prefix along this demand alone. Unused sibling fields
/// are never enumerated, even when their type graph shares large subtrees.
pub(super) fn partition(
    program: &TypedTrees,
    machine: &Machine,
    subject: &ProgressSubject,
) -> Option<ProgressSubject> {
    use typed_trees::types::TypeReferenceNode;
    let parameter = program
        .machine_states(machine)
        .iter()
        .flat_map(|state| program.state_parameters(state))
        .find(|parameter| parameter.symbol == subject.root)?;
    let mut partition = ProgressSubject {
        root: subject.root,
        projections: Vec::new(),
    };
    let mut current = parameter.type_reference;
    let mut visiting = Vec::new();
    for projection in &subject.projections {
        loop {
            match program.type_reference_table.type_reference(current) {
                TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                TypeReferenceNode::Reference { referee, .. }
                    if partition.projections.is_empty() =>
                {
                    current = *referee;
                }
                // A root may borrow an aggregate. A nested reference is one
                // opaque leaf, not a recursively expanded referent snapshot.
                TypeReferenceNode::Reference { .. } => return Some(partition),
                _ => break,
            }
        }
        let Some(data) = super::super::replay_data_type(program, current, machine.symbol) else {
            return Some(partition);
        };
        if visiting.contains(&data.symbol) {
            // Valid recursive proof shapes remain opaque at the repeated type.
            return Some(partition);
        }
        visiting.push(data.symbol);
        let field = program
            .data_members(data)
            .iter()
            .flat_map(|member| match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant),
            })
            .find(|field| field.symbol == *projection)?;
        partition.projections.push(field.symbol);
        current = field.type_reference;
    }
    Some(partition)
}

/// A more specific partition always wins, including when its origin is unknown.
/// Falling back to an enclosing value would resurrect an overwritten field.
pub(super) fn matching_prefix<'places>(
    places: impl Iterator<Item = &'places ProgressSubject>,
    subject: &ProgressSubject,
) -> Option<&'places ProgressSubject> {
    places
        .filter(|place| {
            place.root == subject.root && subject.projections.starts_with(&place.projections)
        })
        .max_by_key(|place| place.projections.len())
}
