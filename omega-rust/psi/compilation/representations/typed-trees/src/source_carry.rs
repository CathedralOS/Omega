//! Derive source type carry requirements from exact declaration metadata.
//! This query does not grant a borrow, prove a body or admit execution.

use crate::TypedTrees;
use crate::data::{DataDefinition, DataField, DataMember, TypeParameter};
use crate::types::TypeReferenceHandle;
use crate::types::TypeReferenceNode;
use symbols::SymbolHandle;

pub struct CarryDerivation<'program> {
    program: &'program TypedTrees,
    parameters: Vec<(SymbolHandle, language_semantics::CarryPolicy)>,
    substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    visiting: Vec<SymbolHandle>,
    requires_borrow_preservation: bool,
    borrow_independent: language_semantics::CarryPolicy,
}

impl<'program> CarryDerivation<'program> {
    pub fn new(program: &'program TypedTrees, parameters: &[TypeParameter]) -> Self {
        let parameters = parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.symbol,
                    parameter
                        .bounds
                        .carry
                        .unwrap_or(language_semantics::CarryPolicy::STRICT),
                )
            })
            .collect();
        Self {
            program,
            parameters,
            substitutions: Vec::new(),
            visiting: Vec::new(),
            requires_borrow_preservation: false,
            borrow_independent: language_semantics::CarryPolicy::PERMISSIVE,
        }
    }

    pub fn derive_requirements(
        mut self,
        type_reference: TypeReferenceHandle,
    ) -> TypeCarryRequirements {
        let structural = self.derive(type_reference);
        TypeCarryRequirements {
            structural,
            borrow_independent: self.borrow_independent,
            requires_borrow_preservation: self.requires_borrow_preservation,
        }
    }

    pub fn derive(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> language_semantics::CarryPolicy {
        use language_semantics::CarryPolicy;

        if !self
            .program
            .type_reference_table
            .contains_type_reference(type_reference)
        {
            self.borrow_independent = self.borrow_independent.intersect(CarryPolicy::STRICT);
            return CarryPolicy::STRICT;
        }
        if let Some(storage) = self.program.layout_storage_arguments(type_reference) {
            return self.derive(storage.semantic_type);
        }
        if self
            .program
            .primitive_type_reference(type_reference)
            .is_some()
        {
            return CarryPolicy::PERMISSIVE;
        }

        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
            .clone()
        {
            TypeReferenceNode::Named { symbol, name } => {
                if let Some((_, argument)) = self
                    .substitutions
                    .iter()
                    .rev()
                    .find(|(candidate, _)| *candidate == symbol && symbol.is_valid())
                {
                    let argument = *argument;
                    return self.derive(argument);
                }
                if let Some((_, policy)) = self
                    .parameters
                    .iter()
                    .rev()
                    .find(|(candidate, _)| *candidate == symbol && symbol.is_valid())
                {
                    if policy.suspension == language_semantics::CarrySuspension::Forbidden {
                        self.requires_borrow_preservation = true;
                    }
                    self.borrow_independent = self.borrow_independent.intersect(*policy);
                    return *policy;
                }
                self.derive_named_data(symbol, name.as_str(), None)
            }
            TypeReferenceNode::Constrained { base_type, .. } => self.derive(base_type),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if length == crate::types::FixedArrayLength::Literal(0) {
                    CarryPolicy::PERMISSIVE
                } else {
                    self.derive(element_type)
                }
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => {
                let arguments = self
                    .program
                    .type_reference_table
                    .type_reference_handles(arguments)
                    .to_vec();
                self.derive_named_data(base_symbol, base_name.as_str(), Some(&arguments))
            }
            TypeReferenceNode::Callable { contract } => {
                if self.program.runtime_callable_contracts.is_valid(contract) {
                    CarryPolicy::PERMISSIVE
                } else {
                    self.borrow_independent =
                        self.borrow_independent.intersect(CarryPolicy::STRICT);
                    CarryPolicy::STRICT
                }
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {
                CarryPolicy::PERMISSIVE
            }
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                self.requires_borrow_preservation = true;
                CarryPolicy::STRICT
            }
            TypeReferenceNode::DynamicTrait { .. } => {
                self.borrow_independent = self.borrow_independent.intersect(CarryPolicy::STRICT);
                CarryPolicy::STRICT
            }
        }
    }

    pub fn derive_named_data(
        &mut self,
        symbol: SymbolHandle,
        _name: &str,
        arguments: Option<&[TypeReferenceHandle]>,
    ) -> language_semantics::CarryPolicy {
        use language_semantics::CarryPolicy;

        let Some(definition) = self.program.unique_data_definition_by_symbol(symbol) else {
            self.borrow_independent = self.borrow_independent.intersect(CarryPolicy::STRICT);
            return CarryPolicy::STRICT;
        };
        if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
            || self.visiting.contains(&definition.symbol)
        {
            self.borrow_independent = self.borrow_independent.intersect(CarryPolicy::STRICT);
            return CarryPolicy::STRICT;
        }

        self.visiting.push(definition.symbol);
        let parameter_len = self.parameters.len();
        let substitution_len = self.substitutions.len();
        let definition_parameters = self.program.data_type_parameters(definition);
        for parameter in definition_parameters {
            self.parameters.push((
                parameter.symbol,
                parameter.bounds.carry.unwrap_or(CarryPolicy::STRICT),
            ));
        }
        if let Some(arguments) = arguments {
            self.substitutions.extend(
                definition_parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
        }

        let mut field_types = Vec::new();
        for_each_stored_field(self.program, definition, &mut |field, _| {
            field_types.push(field.type_reference);
        });
        let mut effective = definition
            .properties
            .carry
            .unwrap_or(CarryPolicy::PERMISSIVE);
        self.borrow_independent = self.borrow_independent.intersect(effective);
        for field_type in field_types {
            effective = effective.intersect(self.derive(field_type));
        }

        self.substitutions.truncate(substitution_len);
        self.parameters.truncate(parameter_len);
        self.visiting.pop();
        effective
    }
}

pub fn for_each_stored_field(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    visit: &mut impl FnMut(&DataField, Option<&str>),
) {
    for member in program.data_members(data_definition) {
        match member {
            DataMember::Field(field) => visit(field, None),
            DataMember::Variant(variant) => {
                for field in program.data_payload_fields(variant) {
                    visit(field, Some(variant.name.as_str()));
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeCarryRequirements {
    pub structural: language_semantics::CarryPolicy,
    pub borrow_independent: language_semantics::CarryPolicy,
    pub requires_borrow_preservation: bool,
}

pub fn effective_data_carry_policy(
    program: &TypedTrees,
    definition: &DataDefinition,
) -> language_semantics::CarryPolicy {
    CarryDerivation::new(program, &[]).derive_named_data(
        definition.symbol,
        definition.name.as_str(),
        None,
    )
}
pub fn effective_type_carry_policy(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    reference: TypeReferenceHandle,
) -> language_semantics::CarryPolicy {
    effective_type_carry_requirements(program, parameters, reference).structural
}
pub fn effective_type_carry_requirements(
    program: &TypedTrees,
    parameters: &[TypeParameter],
    reference: TypeReferenceHandle,
) -> TypeCarryRequirements {
    CarryDerivation::new(program, parameters).derive_requirements(reference)
}
