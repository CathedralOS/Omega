use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees as typed;

use crate::type_reference::constraints::lower_type_constraint_node_span_with_context;

pub(super) fn lower_type_reference_handle_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    let result: Result<_, Diagnostic> = match type_reference {
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
                    lifetime: reference
                        .lifetime
                        .as_ref()
                        .map(crate::lowerer::name::lower_name),
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
            let reference = typed_trees.type_reference_table.insert(
                typed::types::TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                },
            );
            Ok(reference)
        }
        resolved::types::TypeReference::FixedArray(fixed_array) => {
            let element_type = lower_type_reference_handle_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(fixed_array.element_type),
                exposure,
            )?;
            lower_fixed_array_type(
                source_trees,
                typed_trees,
                element_type,
                &fixed_array.length,
                exposure,
            )
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
            // Nested applications publish child spans into this same arena.
            // Finish recursion before inserting the parent's contiguous roster.
            let source_arguments = source_trees.child_type_references(generic.arguments);
            let mut lowered_arguments = Vec::with_capacity(source_arguments.len());
            for argument in source_arguments {
                let argument = lower_type_reference_handle_with_context(
                    source_trees,
                    typed_trees,
                    argument,
                    exposure,
                )?;
                lowered_arguments.push(argument);
            }
            let arguments = typed_trees
                .type_reference_table
                .insert_type_reference_handles(lowered_arguments);

            super::retain_type_reference_selection(
                source_trees,
                typed_trees,
                &generic.base_name,
                generic.base_symbol,
                exposure,
                language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
            )?;
            Ok(typed_trees
                .type_reference_table
                .insert(typed::types::TypeReferenceNode::Generic {
                    base_symbol: generic.base_symbol,
                    base_name: crate::lowerer::name::lower_name(&generic.base_name),
                    lifetime_arguments: generic
                        .lifetime_arguments
                        .iter()
                        .map(crate::lowerer::name::lower_name)
                        .collect(),
                    arguments,
                }))
        }
        resolved::types::TypeReference::ConstExpression(expression) => {
            let expression = crate::expressions::expression::lower_expression_handle_from_table(
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
            arguments,
        } => {
            // Arguments publish child spans into the same arena; finish their
            // recursion before inserting this node's contiguous roster.
            let source_arguments = source_trees.child_type_references(*arguments);
            let mut lowered_arguments = Vec::with_capacity(source_arguments.len());
            for argument in source_arguments {
                lowered_arguments.push(lower_type_reference_handle_with_context(
                    source_trees,
                    typed_trees,
                    argument,
                    exposure,
                )?);
            }
            let arguments = typed_trees
                .type_reference_table
                .insert_type_reference_handles(lowered_arguments);
            super::lower_dynamic_trait_type_reference(
                source_trees,
                typed_trees,
                *symbol,
                name,
                arguments,
                exposure,
            )
        }
        resolved::types::TypeReference::Named { symbol, name } => {
            super::lower_named_type_reference(
                source_trees,
                typed_trees,
                *symbol,
                name,
                exposure,
            )
        }
        resolved::types::TypeReference::Unit => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Unit)),
    };
    let reference = result?;
    super::retain_computed_result_types(&source_trees.symbols, typed_trees, reference, None);
    Ok(reference)
}

pub(super) fn lower_fixed_array_type(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    element_type: typed::types::TypeReferenceHandle,
    length: &resolved::types::FixedArrayLength,
    exposure: Option<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
    >,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    let mut origin = None;
    let length = match length {
        resolved::types::FixedArrayLength::Literal(value) => {
            typed::types::FixedArrayLength::Literal(*value)
        }
        resolved::types::FixedArrayLength::ConstParameter { symbol, name } => {
            if symbol.is_valid()
                && source_trees.symbols.get(*symbol).kind == symbols::SymbolKind::Const
            {
                // Resolution already chose the declaration in the occurrence's
                // source scope and checked its initializer. Consume that exact
                // value, not the spelling or a fresh evaluation in caller scope.
                // Both signature and body type routes retain selection custody
                // before erasing the constant into an ordinary fixed extent.
                let declaration = source_trees
                    .roots
                    .const_declarations
                    .iter()
                    .find(|declaration| declaration.symbol == *symbol)
                    .ok_or_else(|| {
                        Diagnostic::error("array length constant has no resolved declaration")
                            .with_source_span(name.source_span())
                    })?;
                let value = declaration
                    .evaluated_value
                    .as_ref()
                    .and_then(|value| value.encoding())
                    .and_then(|encoding| {
                        language_semantics::const_value::CanonicalConstValue::new("", encoding, "")
                            .decode_encoding()
                    });
                let Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                    value,
                    ..
                }) = value
                else {
                    return Err(Diagnostic::error(format!(
                        "fixed-array length `{name}` requires an evaluated integer constant"
                    ))
                    .with_source_span(name.source_span()));
                };
                let value = usize::try_from(value).map_err(|_| Diagnostic::error(format!(
                    "fixed-array length `{name}` must be nonnegative and fit the supported array extent"
                )).with_source_span(name.source_span()))?;
                super::retain_type_reference_selection(
                    source_trees,
                    typed_trees,
                    name,
                    *symbol,
                    exposure,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment,
                )?;
                origin = Some(typed::types::TypeConstantOrigin {
                    declaration: *symbol,
                    reference: name.source_span(),
                });
                typed::types::FixedArrayLength::Literal(value)
            } else {
                typed::types::FixedArrayLength::ConstParameter {
                    symbol: *symbol,
                    name: crate::lowerer::name::lower_name(name),
                }
            }
        }
        resolved::types::FixedArrayLength::ConstCall { machine, name } => {
            typed::types::FixedArrayLength::ConstCall {
                machine: *machine,
                name: crate::lowerer::name::lower_name(name),
                source_span: name.source_span(),
            }
        }
    };
    let reference =
        typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::FixedArray {
                element_type,
                length,
            });
    // The numeric extent is sufficient for layout, not for declaration
    // reachability or evaluation custody. Keep the selected constant attached
    // to this type use after folding, including when the type is copied.
    typed_trees
        .type_reference_table
        .attach_constant_origins(reference, origin)
        .map_err(Diagnostic::error)?;
    Ok(reference)
}
