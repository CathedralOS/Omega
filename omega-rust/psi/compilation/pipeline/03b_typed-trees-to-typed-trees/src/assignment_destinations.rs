//! Prepare receiving types after destination declarations and selectors settle.
//!
//! A range assignment receives a complete array whose extent is the selected
//! width. That type differs from both the backing collection and the borrowed
//! view a range expression may produce. Retain it on the assignment, not on its
//! RHS: construction checks still owe every element predicate, copied values
//! keep their own producer types, and bounds/access remain independent checks.
//! Unknown widths stay unavailable rather than borrowing the source's extent.

use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

enum ReceivingType {
    Declared(TypeReferenceHandle),
    Window {
        element_type: TypeReferenceHandle,
        length: usize,
    },
}

pub(crate) fn prepare(program: &mut TypedTrees) {
    let mut destinations = Vec::new();
    let mut bounds = validation::ImmutableBoundLookup::new(program);
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for handle in state.statement_nodes.handles() {
                let Some(assignment) = program.statement_table.statement(handle).as_assignment()
                else {
                    continue;
                };
                // Preparation can run again on a transformed private graph.
                // Recompute from its current target, never from the old cache.
                let assignment = typed_trees::statement::TableAssignment {
                    receiving_type: TypeReferenceHandle::invalid(),
                    ..*assignment
                };
                let is_window = program
                    .expression_table
                    .indexed_node(assignment.target)
                    .and_then(|indexed| program.expression_table.range_node(indexed.index))
                    .is_some();
                let receiving = if is_window {
                    validation::array_window_shape(
                        program,
                        machine,
                        state,
                        assignment.target,
                        &mut bounds,
                    )
                    .and_then(|shape| {
                        let length = shape.length()?;
                        shape
                            .element_type
                            .is_valid()
                            .then_some(ReceivingType::Window {
                                element_type: shape.element_type,
                                length,
                            })
                    })
                } else {
                    validation::assignment_stored_value_type(program, machine, state, &assignment)
                        .map(ReceivingType::Declared)
                };
                destinations.push((handle, receiving));
            }
        }
    }
    for (handle, receiving) in destinations {
        let reference = match receiving {
            Some(ReceivingType::Declared(reference)) => reference,
            Some(ReceivingType::Window {
                element_type,
                length,
            }) => {
                let node = TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                };
                let existing = program
                    .type_reference_table
                    .type_reference_rows()
                    .find_map(|(reference, candidate)| (candidate == &node).then_some(reference));
                existing.unwrap_or_else(|| program.type_reference_table.insert(node))
            }
            None => TypeReferenceHandle::invalid(),
        };
        if let StatementNode::Assignment(assignment) = program.statement_table.statement_mut(handle)
        {
            assignment.receiving_type = reference;
        }
    }
}
