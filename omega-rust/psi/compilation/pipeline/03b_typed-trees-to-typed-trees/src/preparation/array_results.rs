//! Retain the complete type of an anonymous array before checking freezes it.
//!
//! A collection projection can know its element without knowing its complete
//! value type. Ownership needs the latter: the authored extent determines each
//! child's claim path. Derive it from completed element results after final call
//! binding, never from a same-shaped declaration or the consumer's formal type.
//! No ownership, membership or loan permission is established by this derivation.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

pub(super) fn retain_array_result_types(program: &mut TypedTrees) {
    let mut occurrences = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut expressions = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
            }
            let mut visited = Vec::new();
            for expression in expressions {
                // A child may be shared by several parents. Actual postorder,
                // not reversed preorder, ensures it is derived first.
                validation::expression_traversal::visit_post_order_with_handles(
                    program,
                    expression,
                    &mut visited,
                    |expression, node| {
                        if matches!(node, ExpressionNode::ArrayLiteral(_)) {
                            occurrences.push((machine.symbol, state.symbol, expression));
                        }
                    },
                );
            }
        }
    }
    for (machine_symbol, state_symbol, expression) in occurrences {
        let derived = (|| {
            let machine = program.machine_by_symbol(machine_symbol)?;
            let state = program.state_by_symbol(state_symbol)?;
            let elements = program.expression_table.array_literal_node(expression)?;
            let elements = program.expression_table.expression_handles(*elements);
            let references = elements
                .iter()
                .map(|element| {
                    validation::expression_result_type_reference(program, machine, state, *element)
                })
                .collect::<Vec<_>>();
            let element_type = validation::join_result_type_references(
                program,
                &references,
                &vec![false; references.len()],
            )?;
            Some((element_type, elements.len()))
        })();
        let Some((element_type, length)) = derived else {
            // A private execution graph may be prepared again after call
            // rebinding. Unresolved new children cannot retain the old type.
            program.expression_table.set_array_literal_type_reference(
                expression,
                typed_trees::types::TypeReferenceHandle::invalid(),
            );
            continue;
        };
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            });
        program
            .expression_table
            .set_array_literal_type_reference(expression, reference);
    }
}
