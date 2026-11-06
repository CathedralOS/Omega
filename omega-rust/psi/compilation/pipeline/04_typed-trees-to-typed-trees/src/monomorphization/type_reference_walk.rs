//! The shared type-reference tree walk: every nested reference enqueues once,
//! and the caller may stop the walk early.

use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Depth-first traversal of the type references nested under `roots`. `on_node`
/// inspects each visited node before its children enqueue; returning true
/// stops the walk and propagates true.
pub(crate) fn for_each_type_reference(
    program: &dyn ::validation::CheckingInput,
    roots: &[TypeReferenceHandle],
    mut on_node: impl FnMut(&dyn ::validation::CheckingInput, &TypeReferenceNode) -> bool,
) -> bool {
    let mut pending = roots.to_vec();
    let mut visited = std::collections::HashSet::new();
    while let Some(handle) = pending.pop() {
        if !handle.is_valid() || !visited.insert(handle) {
            continue;
        }
        let node = program.type_reference_table.type_reference(handle);
        if on_node(program, node) {
            return true;
        }
        match node {
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            pending.extend(domain.arguments.iter().copied());
                        }

                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. } => pending.push(*element_type),
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            ),
            _ => {}
        }
    }
    false
}
