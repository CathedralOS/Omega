use crate::expression::lower_expression;
use crate::program::Lowerer;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees::SymbolResolvedTrees;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_type_reference(
    lowerer: &mut Lowerer,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReference, Diagnostic> {
    lower_type_reference_with_context(lowerer.source_trees, &mut lowerer.typed_trees, type_reference)
}

fn lower_type_reference_with_context(
    source_trees: &SymbolResolvedTrees,
    typed_trees: &mut typed::TypedTrees,
    type_reference: &resolved::types::TypeReference,
) -> Result<typed::types::TypeReference, Diagnostic> {
    match type_reference {
        resolved::types::TypeReference::Reference(reference) => {
            Ok(typed::types::TypeReference::Reference {
                referee: Box::new(lower_type_reference_with_context(
                    source_trees,
                    typed_trees,
                    source_trees.child_type_reference(reference.referee),
                )?),
                is_mutable: reference.is_mutable,
            })
        }
        resolved::types::TypeReference::Constrained(constrained) => {
            Ok(typed::types::TypeReference::Constrained {
                base_type: Box::new(lower_type_reference_with_context(
                    source_trees,
                    typed_trees,
                    source_trees.child_type_reference(constrained.base_type),
                )?),
                constraints: lower_type_constraints_with_context(
                    source_trees,
                    typed_trees,
                    constrained.constraints,
                )?,
            })
        }
        resolved::types::TypeReference::FixedArray(fixed_array) => {
            Ok(typed::types::TypeReference::FixedArray {
                element_type: Box::new(lower_type_reference_with_context(
                    source_trees,
                    typed_trees,
                    source_trees.child_type_reference(fixed_array.element_type),
                )?),
                length: fixed_array.length,
            })
        }
        resolved::types::TypeReference::Slice(slice) => Ok(typed::types::TypeReference::Slice {
            element_type: Box::new(lower_type_reference_with_context(
                source_trees,
                typed_trees,
                source_trees.child_type_reference(slice.element_type),
            )?),
        }),
        resolved::types::TypeReference::Generic(generic) => {
            let mut arguments = HandleSpan::empty();
            for argument in source_trees.child_type_references(generic.arguments) {
                let argument =
                    lower_type_reference_with_context(source_trees, typed_trees, argument)?;
                typed_trees.push_type_reference_argument(&mut arguments, argument);
            }

            Ok(typed::types::TypeReference::Generic {
                base_symbol: generic.base_symbol,
                base_name: crate::name::lower_name(&generic.base_name),
                arguments,
            })
        }
        resolved::types::TypeReference::Named { symbol, name } => {
            Ok(typed::types::TypeReference::Named {
                symbol: *symbol,
                name: crate::name::lower_name(name),
            })
        }
        resolved::types::TypeReference::SelfType { symbol } => {
            Ok(typed::types::TypeReference::Named {
                symbol: *symbol,
                name: typed::name::ProgramName::generated("Self"),
            })
        }
        resolved::types::TypeReference::Unit => Ok(typed::types::TypeReference::Unit),
    }
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
        let constraint = lower_type_constraint(constraint)?;
        typed_trees
            .type_constraints
            .append_to_span(&mut span, constraint);
    }

    Ok(span)
}

fn lower_type_constraint(
    constraint: &resolved::types::TypeConstraint,
) -> Result<typed::types::TypeConstraint, Diagnostic> {
    match constraint {
        resolved::types::TypeConstraint::Named(name) => Ok(typed::types::TypeConstraint::Named(
            crate::name::lower_name(name),
        )),
        resolved::types::TypeConstraint::Range { minimum, maximum } => {
            Ok(typed::types::TypeConstraint::Range {
                minimum: lower_expression(minimum)?,
                maximum: lower_expression(maximum)?,
            })
        }
    }
}
