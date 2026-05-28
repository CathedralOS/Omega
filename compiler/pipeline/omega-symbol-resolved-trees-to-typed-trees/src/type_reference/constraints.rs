use crate::expression::lower_expression_handle_from_table;
use crate::program::Lowerer;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees as typed;

pub(super) fn lower_type_constraint_node_span_from_table(
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
) -> Result<HandleSpan<typed::types::TypeConstraintNode>, Diagnostic> {
    lower_type_constraints_with_context(lowerer.source_trees, &mut lowerer.typed_trees, constraints)
}

pub(super) fn lower_type_constraint_node_span_with_context(
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

fn lower_type_constraints_with_context(
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
