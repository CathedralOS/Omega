use super::*;
use psi_typed_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    handle: TypeReferenceHandle,
    node: TypeReferenceNode,
    arguments: Vec<TypeReferenceHandle>,
    constraints: Vec<TypeConstraintNode>,
}

pub(super) fn capture(
    builder: &mut Builder<'_>,
    handle: TypeReferenceHandle,
) -> Result<(), Vec<Diagnostic>> {
    let table = &builder.program.type_reference_table;
    if !table.contains_type_reference(handle) {
        return Err(rejected("a stale nonzero type-reference handle"));
    }
    let node = table.type_reference(handle);
    let mut arguments = Vec::new();
    let mut constraints = Vec::new();
    match node {
        TypeReferenceNode::Reference { referee, .. } => arguments.push(*referee),
        TypeReferenceNode::Constrained {
            base_type,
            constraints: span,
        } => {
            arguments.push(*base_type);
            builder.charge(span.count() as usize)?;
            constraints.extend_from_slice(table.constraints(*span));
            for constraint in &constraints {
                match constraint {
                    TypeConstraintNode::Range { minimum, maximum } => {
                        builder.expression(*minimum)?;
                        builder.expression(*maximum)?;
                    }
                    TypeConstraintNode::Domain(domain) => {
                        builder.charge(domain.arguments.len())?;
                        arguments.extend_from_slice(&domain.arguments);
                        builder.symbol(domain.symbol)?;
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            arguments.push(*element_type);
            match length {
                FixedArrayLength::ConstParameter { symbol, .. } => builder.symbol(*symbol)?,
                FixedArrayLength::ConstCall { .. } => {
                    return Err(rejected("an unsettled array-length call"));
                }
                FixedArrayLength::Literal(_) => {}
            }
        }
        TypeReferenceNode::Slice { element_type } => arguments.push(*element_type),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments: span,
            ..
        } => {
            builder.symbol(*base_symbol)?;
            builder.charge(span.count() as usize)?;
            arguments.extend_from_slice(table.type_reference_handles(*span));
        }
        TypeReferenceNode::ConstExpression(expression) => builder.expression(*expression)?,
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance,
            ..
        } => {
            builder.symbol(*symbol)?;
            if let Some(symbol) = conformance {
                builder.symbol(*symbol)?;
            }
        }
        TypeReferenceNode::Named { symbol, .. } => builder.symbol(*symbol)?,
        TypeReferenceNode::Unit => {}
    }
    for argument in &arguments {
        builder.type_reference(*argument)?;
    }
    builder.result.types.push(Snapshot {
        handle,
        node: node.clone(),
        arguments,
        constraints,
    });
    Ok(())
}
