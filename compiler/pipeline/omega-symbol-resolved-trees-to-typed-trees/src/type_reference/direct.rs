use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees as typed;

use crate::type_reference::constraints::lower_type_constraint_node_span_with_context;

pub(super) fn lower_type_reference_handle_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    match type_reference {
        resolved::types::TypeReference::Reference(reference) => {
            let referee = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(reference.referee),
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Reference {
                    referee,
                    is_mutable: reference.is_mutable,
                    is_relaxed: reference.is_relaxed,
                },
            ))
        }
        resolved::types::TypeReference::Constrained(constrained) => {
            let base_type = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(constrained.base_type),
            )?;
            let constraints = lower_type_constraint_node_span_with_context(
                source_trees,
                typed_trees,
                constrained.constraints,
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                },
            ))
        }
        resolved::types::TypeReference::FixedArray(fixed_array) => {
            let element_type = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(fixed_array.element_type),
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: lower_fixed_array_length(&fixed_array.length),
                },
            ))
        }
        resolved::types::TypeReference::Slice(slice) => {
            let element_type = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(slice.element_type),
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Slice { element_type }))
        }
        resolved::types::TypeReference::Generic(generic) => {
            let mut arguments = HandleSpan::empty();
            for argument in source_trees.child_type_references(generic.arguments) {
                let argument =
                    lower_type_reference_handle_with_context(source_trees, typed_trees, argument)?;
                typed_trees
                    .type_reference_table
                    .push_type_reference_handle(&mut arguments, argument);
            }

            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Generic {
                    base_symbol: generic.base_symbol,
                    base_name: crate::name::lower_name(&generic.base_name),
                    arguments,
                }))
        }
        resolved::types::TypeReference::DynamicTrait { symbol, name } => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::DynamicTrait {
                symbol: *symbol,
                name: crate::name::lower_name(name),
            })),
        resolved::types::TypeReference::Named { symbol, name } => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named {
                symbol: *symbol,
                name: crate::name::lower_name(name),
            })),
        resolved::types::TypeReference::SelfType { symbol } => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named {
                symbol: *symbol,
                name: typed::name::Identifier::generated_static("Self"),
            })),
        resolved::types::TypeReference::Unit => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Unit)),
    }
}

pub(super) fn lower_fixed_array_length(
    length: &resolved::types::FixedArrayLength,
) -> typed::types::FixedArrayLength {
    match length {
        resolved::types::FixedArrayLength::Literal(value) => {
            typed::types::FixedArrayLength::Literal(*value)
        }
        resolved::types::FixedArrayLength::ConstParameter { symbol, name } => {
            typed::types::FixedArrayLength::ConstParameter {
                symbol: *symbol,
                name: crate::name::lower_name(name),
            }
        }
    }
}
