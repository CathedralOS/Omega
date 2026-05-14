use crate::expression::lower_expression;
use crate::program::Lowerer;
use crate::statement::lower_expression_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::types::{TypeConstraint, TypeReference};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_type_constraints(
    lowerer: &mut Lowerer,
    constraints: &[syntax::types::TypeConstraint],
) -> Result<HandleSpan<TypeConstraint>, Diagnostic> {
    let lowered = constraints
        .iter()
        .map(lower_type_constraint)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lowerer.program.type_constraints.insert_many(lowered))
}

pub(crate) fn lower_type_reference_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    match syntax_trees.type_references.type_reference(type_reference) {
        syntax::types::TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => Ok(TypeReference::Reference {
            referee: Box::new(lower_type_reference_handle(
                lowerer,
                syntax_trees,
                *referee,
            )?),
            is_mutable: *is_mutable,
        }),
        syntax::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => Ok(TypeReference::Constrained {
            base_type: Box::new(lower_type_reference_handle(
                lowerer,
                syntax_trees,
                *base_type,
            )?),
            constraints: lower_type_constraint_handles(lowerer, syntax_trees, *constraints)?,
        }),
        syntax::types::TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => Ok(TypeReference::FixedArray {
            element_type: Box::new(lower_type_reference_handle(
                lowerer,
                syntax_trees,
                *element_type,
            )?),
            length: *length,
        }),
        syntax::types::TypeReferenceNode::Slice { element_type } => Ok(TypeReference::Slice {
            element_type: Box::new(lower_type_reference_handle(
                lowerer,
                syntax_trees,
                *element_type,
            )?),
        }),
        syntax::types::TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => Ok(TypeReference::Generic {
            base_symbol: SymbolHandle::invalid(),
            base_name: crate::name::lower_name(base_name),
            arguments: syntax_trees
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| lower_type_reference_handle(lowerer, syntax_trees, *argument))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        syntax::types::TypeReferenceNode::Named(name) => Ok(TypeReference::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
        }),
        syntax::types::TypeReferenceNode::Unit => Ok(TypeReference::Unit),
    }
}

pub(crate) fn lower_type_constraint_handles(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    constraints: HandleSpan<syntax::types::TypeConstraintNode>,
) -> Result<HandleSpan<TypeConstraint>, Diagnostic> {
    let lowered = syntax_trees
        .type_references
        .constraints(constraints)
        .iter()
        .map(|constraint| lower_type_constraint_handle(syntax_trees, constraint))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lowerer.program.type_constraints.insert_many(lowered))
}

fn lower_type_constraint_handle(
    syntax_trees: &SyntaxTrees,
    constraint: &syntax::types::TypeConstraintNode,
) -> Result<TypeConstraint, Diagnostic> {
    match constraint {
        syntax::types::TypeConstraintNode::Named(name) => {
            Ok(TypeConstraint::Named(crate::name::lower_name(name)))
        }
        syntax::types::TypeConstraintNode::Range { minimum, maximum } => {
            Ok(TypeConstraint::Range {
                minimum: lower_expression_handle(syntax_trees, *minimum)?,
                maximum: lower_expression_handle(syntax_trees, *maximum)?,
            })
        }
    }
}

fn lower_type_constraint(
    constraint: &syntax::types::TypeConstraint,
) -> Result<TypeConstraint, Diagnostic> {
    match constraint {
        syntax::types::TypeConstraint::Named(name) => {
            Ok(TypeConstraint::Named(crate::name::lower_name(name)))
        }
        syntax::types::TypeConstraint::Range { minimum, maximum } => Ok(TypeConstraint::Range {
            minimum: lower_expression(minimum)?,
            maximum: lower_expression(maximum)?,
        }),
    }
}
