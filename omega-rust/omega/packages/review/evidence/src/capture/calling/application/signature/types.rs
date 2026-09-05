//! Scope-aware projection-only instantiation of checked signature types.

use super::rejected;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn instantiate(
    compilation: &mut CheckedCompilation,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[(Identifier, Identifier)],
    depth: usize,
) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
    if !reference.is_valid() {
        return Ok(reference);
    }
    if depth >= 64 {
        return Err(rejected(
            "calling signature type exceeds the projection depth limit",
        ));
    }
    let node = compilation
        .type_reference_table
        .type_reference(reference)
        .clone();
    let lifetime = |name: Identifier| {
        lifetimes
            .iter()
            .find(|(source, _)| *source == name)
            .map(|(_, target)| target.clone())
            .ok_or_else(|| rejected("calling signature has an unbound lifetime"))
    };
    let node = match node {
        TypeReferenceNode::Named { symbol, name } => {
            if let Some((_, actual)) = substitutions
                .iter()
                .find(|(parameter, _)| *parameter == symbol)
            {
                return Ok(*actual);
            }
            if let Some(layout) = compilation
                .plan_laid_layouts
                .iter()
                .find(|layout| layout.data_symbol == symbol)
                .cloned()
            {
                let schema = compilation
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == layout.schema_symbol)
                    .cloned()
                    .ok_or_else(|| rejected("plan-laid signature type lost its exact schema"))?;
                let schema_reference = match schema.generic_instance {
                    Some(reference) => {
                        instantiate(compilation, reference, substitutions, lifetimes, depth + 1)?
                    }
                    None => {
                        compilation
                            .typed
                            .type_reference_table
                            .insert(TypeReferenceNode::Named {
                                symbol: schema.symbol,
                                name: schema.name,
                            })
                    }
                };
                let base_name =
                    Identifier::generated(compilation.symbols.name(layout.policy_symbol));
                let arguments = compilation
                    .typed
                    .type_reference_table
                    .insert_type_reference_handles([schema_reference]);
                TypeReferenceNode::Generic {
                    base_symbol: layout.policy_symbol,
                    base_name,
                    lifetime_arguments: Vec::new(),
                    arguments,
                }
            } else {
                TypeReferenceNode::Named { symbol, name }
            }
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: region,
        } => TypeReferenceNode::Reference {
            referee: instantiate(compilation, referee, substitutions, lifetimes, depth + 1)?,
            access,
            lifetime: region.map(lifetime).transpose()?,
        },
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base_type =
                instantiate(compilation, base_type, substitutions, lifetimes, depth + 1)?;
            let mut constraints = compilation
                .type_reference_table
                .constraints(constraints)
                .to_vec();
            for constraint in &mut constraints {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    for argument in &mut domain.arguments {
                        *argument = instantiate(
                            compilation,
                            *argument,
                            substitutions,
                            lifetimes,
                            depth + 1,
                        )?;
                    }
                }
            }
            let constraints = compilation
                .typed
                .type_reference_table
                .insert_constraints(constraints);
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => TypeReferenceNode::FixedArray {
            element_type: instantiate(
                compilation,
                element_type,
                substitutions,
                lifetimes,
                depth + 1,
            )?,
            length,
        },
        TypeReferenceNode::Slice { element_type } => TypeReferenceNode::Slice {
            element_type: instantiate(
                compilation,
                element_type,
                substitutions,
                lifetimes,
                depth + 1,
            )?,
        },
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let mut arguments = compilation
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec();
            for argument in &mut arguments {
                *argument =
                    instantiate(compilation, *argument, substitutions, lifetimes, depth + 1)?;
            }
            let arguments = compilation
                .typed
                .type_reference_table
                .insert_type_reference_handles(arguments);
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments: lifetime_arguments
                    .into_iter()
                    .map(lifetime)
                    .collect::<Result<_, _>>()?,
                arguments,
            }
        }
        other => other,
    };
    Ok(compilation.typed.type_reference_table.insert(node))
}
