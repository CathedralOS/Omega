use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_typed_trees as typed;

use crate::type_reference::constraints::lower_type_constraint_node_span_with_context;

pub(super) fn lower_type_reference_handle_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
    exposure: Option<
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    match type_reference {
        resolved::types::TypeReference::Reference(reference) => {
            let referee = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(reference.referee),
                exposure,
            )?;
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Reference {
                    referee,
                    access: reference.access,
                    lifetime: reference.lifetime.as_ref().map(crate::name::lower_name),
                },
            ))
        }
        resolved::types::TypeReference::Constrained(constrained) => {
            let base_type = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(constrained.base_type),
                exposure,
            )?;
            let constraints = lower_type_constraint_node_span_with_context(
                source_trees,
                typed_trees,
                constrained.constraints,
                exposure,
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
                exposure,
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
                exposure,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Slice { element_type }))
        }
        resolved::types::TypeReference::Generic(generic) => {
            let mut arguments = HandleSpan::empty();
            for argument in source_trees.child_type_references(generic.arguments) {
                let argument = lower_type_reference_handle_with_context(
                    source_trees,
                    typed_trees,
                    argument,
                    exposure,
                )?;
                typed_trees
                    .type_reference_table
                    .push_type_reference_handle(&mut arguments, argument);
            }

            super::retain_type_reference_selection(
                source_trees,
                typed_trees,
                &generic.base_name,
                generic.base_symbol,
                exposure,
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Generic {
                    base_symbol: generic.base_symbol,
                    base_name: crate::name::lower_name(&generic.base_name),
                    lifetime_arguments: generic
                        .lifetime_arguments
                        .iter()
                        .map(crate::name::lower_name)
                        .collect(),
                    arguments,
                }))
        }
        resolved::types::TypeReference::ConstExpression(expression) => {
            let expression = crate::expression::lower_expression_handle_from_table(
                &source_trees.tables.bodies.expressions,
                typed_trees,
                *expression,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::ConstExpression(expression)))
        }
        resolved::types::TypeReference::DynamicTrait {
            symbol,
            name,
            conformance,
            conformance_carrier,
            conformance_name,
        } => {
            super::retain_type_reference_selection(
                source_trees,
                typed_trees,
                name,
                *symbol,
                exposure,
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            if let (Some(conformance), Some(conformance_name)) =
                (*conformance, conformance_name.as_ref())
            {
                super::retain_type_reference_selection(
                    source_trees,
                    typed_trees,
                    conformance_name,
                    conformance,
                    exposure,
                    psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance,
                )?;
            }
            Ok(typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::DynamicTrait {
                    symbol: *symbol,
                    name: crate::name::lower_name(name),
                    conformance: *conformance,
                    conformance_carrier: conformance_carrier.as_ref().map(crate::name::lower_name),
                    conformance_name: conformance_name.as_ref().map(crate::name::lower_name),
                },
            ))
        }
        resolved::types::TypeReference::Named { symbol, name } => {
            super::retain_type_reference_selection(
                source_trees,
                typed_trees,
                name,
                *symbol,
                exposure,
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Named {
                    symbol: *symbol,
                    name: crate::name::lower_name(name),
                }))
        }
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
        resolved::types::FixedArrayLength::ConstCall { name } => {
            typed::types::FixedArrayLength::ConstCall {
                name: crate::name::lower_name(name),
                source_span: name.source_span(),
            }
        }
    }
}
