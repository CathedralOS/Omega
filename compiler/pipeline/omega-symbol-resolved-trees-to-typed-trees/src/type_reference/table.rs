use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees as typed;

use crate::type_reference::constraints::lower_type_constraint_node_span_from_table;
use crate::type_reference::direct::lower_fixed_array_length;

pub(super) fn lower_type_reference_handle_from_table_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    match source_trees
        .tables
        .types
        .references
        .type_reference(type_reference)
    {
        resolved::types::TypeReferenceNode::Reference {
            referee,
            is_mutable,
            is_relaxed,
        } => {
            let referee = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *referee,
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Reference {
                    referee,
                    is_mutable: *is_mutable,
                    is_relaxed: *is_relaxed,
                },
            ))
        }
        resolved::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base_type = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *base_type,
            )?;
            let constraints = lower_type_constraint_node_span_from_table(
                source_trees,
                typed_trees,
                *constraints,
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                },
            ))
        }
        resolved::types::TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let element_type = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *element_type,
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: lower_fixed_array_length(length),
                },
            ))
        }
        resolved::types::TypeReferenceNode::Slice { element_type } => {
            let element_type = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *element_type,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Slice { element_type }))
        }
        resolved::types::TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
        } => {
            let mut lowered_arguments = HandleSpan::empty();
            for argument in source_trees
                .tables
                .types
                .references
                .type_reference_handles(*arguments)
            {
                let argument = lower_type_reference_handle_from_table_with_context(
                    source_trees,
                    typed_trees,
                    *argument,
                )?;
                typed_trees
                    .type_reference_table
                    .push_type_reference_handle(&mut lowered_arguments, argument);
            }

            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Generic {
                    base_symbol: *base_symbol,
                    base_name: crate::name::lower_name(base_name),
                    arguments: lowered_arguments,
                }))
        }
        resolved::types::TypeReferenceNode::DynamicTrait { symbol, name } => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::DynamicTrait {
                symbol: *symbol,
                name: crate::name::lower_name(name),
            })),
        resolved::types::TypeReferenceNode::Named { symbol, name } => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named {
                symbol: *symbol,
                name: crate::name::lower_name(name),
            })),
        resolved::types::TypeReferenceNode::SelfType { symbol } => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Named {
                symbol: *symbol,
                name: typed::name::Identifier::generated_static("Self"),
            })),
        resolved::types::TypeReferenceNode::Unit => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Unit)),
    }
}
