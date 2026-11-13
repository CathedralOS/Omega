//! Query properties promised by exact source type declarations and binders.
use crate::{
    TypedTrees,
    data::TypeParameter,
    types::{TypeReferenceHandle, TypeReferenceNode},
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredPropertyRequirement {
    Copy,
    Linear,
    Carry(language_semantics::CarryPolicy),
}

impl std::fmt::Display for DeclaredPropertyRequirement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy => formatter.write_str("copy"),
            Self::Linear => formatter.write_str("linear"),
            Self::Carry(policy) => write!(formatter, "{policy}"),
        }
    }
}

pub fn type_satisfies_declared_property(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    reference: TypeReferenceHandle,
    property: DeclaredPropertyRequirement,
) -> bool {
    match property {
        DeclaredPropertyRequirement::Carry(required) => {
            crate::source_carry::CarryDerivation::new(program, parameters)
                .derive(reference)
                .permits(required)
        }
        DeclaredPropertyRequirement::Copy | DeclaredPropertyRequirement::Linear => {
            structural_property(
                program,
                parameters,
                reference,
                &[],
                property,
                &mut Vec::new(),
            )
        }
    }
}

pub fn substituted_type_satisfies_copy(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    structural_property(
        program,
        &[],
        reference,
        substitutions,
        DeclaredPropertyRequirement::Copy,
        &mut Vec::new(),
    )
}

fn structural_property(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    property: DeclaredPropertyRequirement,
    active: &mut Vec<TypeReferenceHandle>,
) -> bool {
    if !program.contains_type_reference(reference) || active.contains(&reference) {
        return false;
    }
    if let Some(storage) = program.layout_storage_arguments(reference) {
        if storage.semantic_type != reference {
            return structural_property(
                program,
                parameters,
                storage.semantic_type,
                substitutions,
                property,
                active,
            );
        }
    }
    if program.primitive_type_reference(reference).is_some() {
        return property == DeclaredPropertyRequirement::Copy;
    }
    active.push(reference);
    let answer = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Callable { contract } => {
            property == DeclaredPropertyRequirement::Copy
                && program.runtime_callable_contracts.is_valid(*contract)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, actual)) = substitutions
                .iter()
                .rev()
                .find(|(formal, _)| formal == symbol)
            {
                *actual != reference
                    && structural_property(
                        program,
                        parameters,
                        *actual,
                        substitutions,
                        property,
                        active,
                    )
            } else if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == *symbol)
            {
                multiplicity_matches(parameter.bounds.multiplicity, property)
            } else {
                program
                    .data_definition_by_symbol(*symbol)
                    .is_some_and(|definition| {
                        multiplicity_matches(definition.properties.multiplicity, property)
                    })
            }
        }
        TypeReferenceNode::Reference { access, .. } => {
            property == DeclaredPropertyRequirement::Copy
                && *access == language_core::ReferenceAccess::Shared
        }
        TypeReferenceNode::Constrained { base_type, .. } => structural_property(
            program,
            parameters,
            *base_type,
            substitutions,
            property,
            active,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            (property == DeclaredPropertyRequirement::Copy
                && program.closed_fixed_array_length(reference) == Some(0))
                || structural_property(
                    program,
                    parameters,
                    *element_type,
                    substitutions,
                    property,
                    active,
                )
        }
        TypeReferenceNode::Generic { base_symbol, .. } => program
            .data_definition_by_symbol(*base_symbol)
            .is_some_and(|definition| {
                multiplicity_matches(definition.properties.multiplicity, property)
            }),
        TypeReferenceNode::Unit => property == DeclaredPropertyRequirement::Copy,
        _ => false,
    };
    active.pop();
    answer
}

fn multiplicity_matches(actual: Multiplicity, property: DeclaredPropertyRequirement) -> bool {
    match property {
        DeclaredPropertyRequirement::Copy => actual == Multiplicity::Unrestricted,
        DeclaredPropertyRequirement::Linear => actual == Multiplicity::Linear,
        DeclaredPropertyRequirement::Carry(_) => false,
    }
}

pub fn referenced_type_parameter<'p>(
    program: &TypedTrees,
    parameters: &'p [TypeParameter],
    mut reference: TypeReferenceHandle,
) -> Option<&'p TypeParameter> {
    let mut active = Vec::new();
    loop {
        if !program.contains_type_reference(reference) || active.contains(&reference) {
            return None;
        }
        active.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                return parameters
                    .iter()
                    .find(|parameter| parameter.symbol == *symbol);
            }
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::FixedArray { element_type, .. } => reference = *element_type,
            _ => return None,
        }
    }
}
