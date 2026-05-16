use crate::expression::lower_expression_handle_from_table;
use crate::program::Lowerer;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees as typed;

pub(crate) fn lower_type_reference(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReference, Diagnostic> {
    let type_reference = lower_type_reference_handle_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )?;

    Ok(lowerer.typed_trees.type_reference_table.to_tree(
        type_reference,
        &lowerer.typed_trees.expression_table,
        &mut lowerer.typed_trees.type_constraints,
        &mut lowerer.typed_trees.type_reference_arguments,
    ))
}

pub(crate) fn lower_type_reference_handle_from_table(
    lowerer: &mut Lowerer,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Result<typed::types::TypeReferenceHandle, Diagnostic> {
    lower_type_reference_handle_from_table_with_context(
        lowerer.source_trees,
        &mut lowerer.typed_trees,
        type_reference,
    )
}

fn lower_type_reference_handle_from_table_with_context(
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
                    length: *length,
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
                name: typed::name::ProgramName::generated("Self"),
            })),
        resolved::types::TypeReferenceNode::Unit => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Unit)),
    }
}

fn lower_type_reference_handle_with_context(
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
                    length: fixed_array.length,
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
                name: typed::name::ProgramName::generated("Self"),
            })),
        resolved::types::TypeReference::Unit => Ok(typed_trees
            .type_reference_table
            .insert(typed::types::TypeReferenceNode::Unit)),
    }
}

fn lower_type_constraint_node_span_from_table(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraintNode>,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .references
        .constraints(constraints)
    {
        let constraint = match constraint {
            resolved::types::TypeConstraintNode::Named(name) => {
                typed::types::TypeConstraintNode::Named(crate::name::lower_name(name))
            }
            resolved::types::TypeConstraintNode::Range { minimum, maximum } => {
                typed::types::TypeConstraintNode::Range {
                    minimum: lower_expression_handle_from_table(
                        &source_trees.tables.bodies.expressions,
                        &mut typed_trees.expression_table,
                        *minimum,
                    )?,
                    maximum: lower_expression_handle_from_table(
                        &source_trees.tables.bodies.expressions,
                        &mut typed_trees.expression_table,
                        *maximum,
                    )?,
                }
            }
        };
        typed_trees
            .type_reference_table
            .push_constraint(&mut span, constraint);
    }

    Ok(span)
}

pub(crate) fn lower_type_constraints(
    lowerer: &mut Lowerer,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
) -> Result<HandleSpan<typed::types::TypeConstraint>, Diagnostic> {
    lower_type_constraints_with_context(lowerer.source_trees, &mut lowerer.typed_trees, constraints)
}

fn lower_type_constraints_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
) -> Result<HandleSpan<typed::types::TypeConstraint>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constraints)
    {
        let constraint =
            lower_type_constraint_node_with_context(source_trees, typed_trees, constraint)?;
        let constraint = constraint.to_tree(&typed_trees.expression_table);
        typed_trees
            .type_constraints
            .append_to_span(&mut span, constraint);
    }

    Ok(span)
}

fn lower_type_constraint_node_span_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraints: HandleSpan<resolved::types::TypeConstraint>,
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constraints)
    {
        let constraint =
            lower_type_constraint_node_with_context(source_trees, typed_trees, constraint)?;
        typed_trees
            .type_reference_table
            .push_constraint(&mut span, constraint);
    }

    Ok(span)
}

fn lower_type_constraint_node_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    constraint: &resolved::types::TypeConstraint,
) -> Result<typed::types::TypeConstraintNode, Diagnostic> {
    match constraint {
        resolved::types::TypeConstraint::Named(name) => Ok(
            typed::types::TypeConstraintNode::Named(crate::name::lower_name(name)),
        ),
        resolved::types::TypeConstraint::Range { minimum, maximum } => {
            Ok(typed::types::TypeConstraintNode::Range {
                minimum: lower_expression_handle_from_table(
                    &source_trees.tables.bodies.expressions,
                    &mut typed_trees.expression_table,
                    *minimum,
                )?,
                maximum: lower_expression_handle_from_table(
                    &source_trees.tables.bodies.expressions,
                    &mut typed_trees.expression_table,
                    *maximum,
                )?,
            })
        }
    }
}
