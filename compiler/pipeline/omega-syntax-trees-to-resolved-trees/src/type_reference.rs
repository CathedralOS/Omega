use crate::expression::lower_expression;
use crate::program::Lowerer;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees as syntax;
use omega_resolved_trees::types::{TypeConstraint, TypeReference};

pub(crate) fn lower_type_reference(
    lowerer: &mut Lowerer,
    type_reference: &syntax::types::TypeReference,
) -> Result<TypeReference, Diagnostic> {
    match type_reference {
        syntax::types::TypeReference::Reference {
            referee,
            is_mutable,
        } => Ok(TypeReference::Reference {
            referee: Box::new(lower_type_reference(lowerer, referee)?),
            is_mutable: *is_mutable,
        }),
        syntax::types::TypeReference::Constrained {
            base_type,
            constraints,
        } => Ok(TypeReference::Constrained {
            base_type: Box::new(lower_type_reference(lowerer, base_type)?),
            constraints: lower_type_constraints(lowerer, constraints)?,
        }),
        syntax::types::TypeReference::FixedArray {
            element_type,
            length,
        } => Ok(TypeReference::FixedArray {
            element_type: Box::new(lower_type_reference(lowerer, element_type)?),
            length: *length,
        }),
        syntax::types::TypeReference::Slice { element_type } => Ok(TypeReference::Slice {
            element_type: Box::new(lower_type_reference(lowerer, element_type)?),
        }),
        syntax::types::TypeReference::Generic {
            base_name,
            arguments,
        } => Ok(TypeReference::Generic {
            base_symbol: SymbolHandle::invalid(),
            base_name: crate::name::lower_name(base_name),
            arguments: arguments
                .iter()
                .map(|argument| lower_type_reference(lowerer, argument))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        syntax::types::TypeReference::Named(name) => Ok(TypeReference::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
        }),
        syntax::types::TypeReference::Unit => Ok(TypeReference::Unit),
    }
}

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
