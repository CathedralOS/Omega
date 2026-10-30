use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees as typed;

use crate::type_reference::constraints::lower_type_constraint_node_span_from_table;
use crate::type_reference::direct::lower_fixed_array_type;

/// Lower a type's argument list into the shared table, preserving order.
fn lower_type_argument_handles(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    arguments: HandleSpan<resolved::types::TypeReferenceHandle>,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<HandleSpan<typed::types::TypeReferenceHandle>, Diagnostic> {
    let mut lowered_arguments = HandleSpan::empty();
    for argument in source_trees
        .tables
        .types
        .references
        .type_reference_handles(arguments)
    {
        let argument = lower_type_reference_handle_from_table_with_context(
            source_trees,
            typed_trees,
            *argument,
            exposure,
        )?;
        typed_trees
            .type_reference_table
            .push_type_reference_handle(&mut lowered_arguments, argument);
    }
    Ok(lowered_arguments)
}

pub(super) fn lower_type_reference_handle_from_table_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: resolved::types::TypeReferenceHandle,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    let result: Result<_, Diagnostic> = match source_trees
        .tables
        .types
        .references
        .type_reference(type_reference)
    {
        resolved::types::TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let referee = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *referee,
                exposure,
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Reference {
                    referee,
                    access: *access,
                    lifetime: lifetime.as_ref().map(crate::lowerer::name::lower_name),
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
                exposure,
            )?;
            let constraints = lower_type_constraint_node_span_from_table(
                source_trees,
                typed_trees,
                *constraints,
                exposure,
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
                exposure,
            )?;
            lower_fixed_array_type(source_trees, typed_trees, element_type, length, exposure)
        }
        resolved::types::TypeReferenceNode::Slice { element_type } => {
            let element_type = lower_type_reference_handle_from_table_with_context(
                source_trees,
                typed_trees,
                *element_type,
                exposure,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Slice { element_type }))
        }
        resolved::types::TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let lowered_arguments =
                lower_type_argument_handles(source_trees, typed_trees, *arguments, exposure)?;

            super::retain_type_reference_selection(
                source_trees,
                typed_trees,
                base_name,
                *base_symbol,
                exposure,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Generic {
                    base_symbol: *base_symbol,
                    base_name: crate::lowerer::name::lower_name(base_name),
                    lifetime_arguments: lifetime_arguments
                        .iter()
                        .map(crate::lowerer::name::lower_name)
                        .collect(),
                    arguments: lowered_arguments,
                }))
        }
        resolved::types::TypeReferenceNode::ConstExpression(expression) => {
            let expression = crate::expressions::expression::lower_expression_handle_from_table_with_symbol_substitution(
                Some(source_trees),
                &source_trees.tables.bodies.expressions,
                typed_trees,
                *expression,
                None,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::ConstExpression(expression)))
        }
        resolved::types::TypeReferenceNode::DynamicTrait {
            symbol,
            name,
            arguments,
        } => {
            let lowered_arguments =
                lower_type_argument_handles(source_trees, typed_trees, *arguments, exposure)?;
            super::lower_dynamic_trait_type_reference(
                source_trees,
                typed_trees,
                *symbol,
                name,
                lowered_arguments,
                exposure,
            )
        }
        resolved::types::TypeReferenceNode::Named { symbol, name } => {
            super::lower_named_type_reference(source_trees, typed_trees, *symbol, name, exposure)
        }
        resolved::types::TypeReferenceNode::Unit => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Unit)),
    };
    let reference = result?;
    super::retain_supplied_operand_owner(
        source_trees,
        typed_trees,
        reference,
        source_trees
            .tables
            .types
            .references
            .supplied_operand_owner(type_reference),
    )?;
    super::retain_computed_result_types(&source_trees.symbols, typed_trees, reference, None);
    Ok(reference)
}
