//! Scope-aware projection-only instantiation of checked signature types.

use super::rejected;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn instantiate(
    compilation: &mut TypedTrees,
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
    let lifetime = |name: &Identifier| {
        lifetimes
            .iter()
            .find(|(source, _)| source == name)
            .map(|(_, target)| target.clone())
            .ok_or_else(|| rejected("calling signature has an unbound lifetime"))
    };
    let node = match compilation.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, name: _ } => {
            let symbol = *symbol;
            if let Some((_, actual)) = substitutions
                .iter()
                .find(|(parameter, _)| *parameter == symbol)
            {
                return Ok(*actual);
            }
            let plan = compilation
                .plan_laid_layouts
                .iter()
                .find(|layout| layout.data_symbol == symbol)
                .map(|layout| (layout.schema_symbol, layout.policy_symbol));
            if let Some((schema_symbol, policy_symbol)) = plan {
                let schema = compilation
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == schema_symbol)
                    .map(|definition| {
                        (
                            definition.symbol,
                            definition.name.clone(),
                            definition.generic_instance,
                        )
                    })
                    .ok_or_else(|| rejected("plan-laid signature type lost its exact schema"))?;
                let schema_reference = match schema.2 {
                    Some(reference) => {
                        instantiate(compilation, reference, substitutions, lifetimes, depth + 1)?
                    }
                    None => compilation
                        .type_reference_table
                        .insert(TypeReferenceNode::Named {
                            symbol: schema.0,
                            name: schema.1,
                        }),
                };
                let base_name = Identifier::generated(compilation.symbols.name(policy_symbol));
                let arguments = compilation
                    .type_reference_table
                    .insert_type_reference_handles([schema_reference]);
                TypeReferenceNode::Generic {
                    base_symbol: policy_symbol,
                    base_name,
                    lifetime_arguments: Vec::new(),
                    arguments,
                }
            } else {
                return Ok(reference);
            }
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: region,
        } => {
            let (referee, access, region) = (*referee, *access, region.clone());
            TypeReferenceNode::Reference {
                referee: instantiate(compilation, referee, substitutions, lifetimes, depth + 1)?,
                access,
                lifetime: region.as_ref().map(&lifetime).transpose()?,
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base_type, constraints) = (*base_type, *constraints);
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
        } => {
            let (element_type, length) = (*element_type, length.clone());
            TypeReferenceNode::FixedArray {
                element_type: instantiate(
                    compilation,
                    element_type,
                    substitutions,
                    lifetimes,
                    depth + 1,
                )?,
                length,
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            let element_type = *element_type;
            TypeReferenceNode::Slice {
                element_type: instantiate(
                    compilation,
                    element_type,
                    substitutions,
                    lifetimes,
                    depth + 1,
                )?,
            }
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let (base_symbol, base_name, arguments) = (*base_symbol, base_name.clone(), *arguments);
            let lifetime_arguments = lifetime_arguments
                .iter()
                .map(lifetime)
                .collect::<Result<Vec<_>, _>>()?;
            let mut arguments = compilation
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec();
            for argument in &mut arguments {
                *argument =
                    instantiate(compilation, *argument, substitutions, lifetimes, depth + 1)?;
            }
            let arguments = compilation
                .type_reference_table
                .insert_type_reference_handles(arguments);
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            }
        }
        _ => return Ok(reference),
    };
    Ok(compilation.type_reference_table.insert(node))
}
