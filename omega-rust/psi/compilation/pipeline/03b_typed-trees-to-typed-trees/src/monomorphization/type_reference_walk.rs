//! The shared type-reference tree walk: every nested reference enqueues once,
//! each `Range` constraint's endpoints report to the caller's hook, and either
//! hook may stop the walk early.

use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Depth-first traversal of the type references nested under `roots`. `on_node`
/// inspects each visited node before its children enqueue and `on_range`
/// receives each `Range` constraint's endpoints; returning true from either
/// stops the walk and propagates true.
/// Collect every `(minimum, maximum)` bound-expression pair reachable under
/// `root`'s range constraints.
pub(crate) fn type_reference_endpoints(
    program: &dyn ::validation::CheckingInput,
    root: TypeReferenceHandle,
) -> Vec<(ExpressionHandle, ExpressionHandle)> {
    let mut endpoints = Vec::new();
    for_each_type_reference(
        program,
        &[root],
        |_, _| false,
        |_, minimum, maximum| {
            endpoints.push((minimum, maximum));
            false
        },
    );
    endpoints
}

pub(crate) fn for_each_type_reference(
    program: &dyn ::validation::CheckingInput,
    roots: &[TypeReferenceHandle],
    mut on_node: impl FnMut(&dyn ::validation::CheckingInput, &TypeReferenceNode) -> bool,
    mut on_range: impl FnMut(
        &dyn ::validation::CheckingInput,
        ExpressionHandle,
        ExpressionHandle,
    ) -> bool,
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
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            if on_range(program, *minimum, *maximum) {
                                return true;
                            }
                        }
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                pending.push(*element_type);
                if let typed_trees::types::FixedArrayLength::Expression(reference) = length {
                    pending.push(*reference);
                }
            }
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
