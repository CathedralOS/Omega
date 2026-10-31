use typed_trees::data::{DataDefinition, DataField, DataMember, TypeParameter};
use typed_trees::types::TypeReferenceNode;

pub(super) struct CarryDerivation<'program> {
    program: &'program dyn crate::CheckingInput,
    parameters: Vec<(
        symbols::SymbolHandle,
        String,
        language_semantics::CarryPolicy,
    )>,
    substitutions: Vec<(
        symbols::SymbolHandle,
        String,
        typed_trees::types::TypeReferenceHandle,
    )>,
    visiting: Vec<symbols::SymbolHandle>,
    requires_borrow_preservation: bool,
    borrow_independent: language_semantics::CarryPolicy,
}

impl<'program> CarryDerivation<'program> {
    pub(super) fn new(
        program: &'program dyn crate::CheckingInput,
        parameters: &[TypeParameter],
    ) -> Self {
        let parameters = parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
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

    pub(super) fn derive_requirements(
        mut self,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> super::TypeCarryRequirements {
        let structural = self.derive(type_reference);
        super::TypeCarryRequirements {
            structural,
            borrow_independent: self.borrow_independent,
            requires_borrow_preservation: self.requires_borrow_preservation,
        }
    }

    pub(super) fn derive(
        &mut self,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> language_semantics::CarryPolicy {
        use language_semantics::CarryPolicy;

        if let Some(storage) = self.program.layout_storage_arguments(type_reference) {
            return self.derive(storage.semantic_type);
        }
        if self
            .program
            .type_reference_table
            .primitive_type(type_reference)
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
                if let Some((_, _, argument)) =
                    self.substitutions
                        .iter()
                        .rev()
                        .find(|(candidate, candidate_name, _)| {
                            (*candidate == symbol && symbol.is_valid())
                                || (!symbol.is_valid() && candidate_name == name.as_str())
                        })
                {
                    let argument = *argument;
                    return self.derive(argument);
                }
                if let Some((_, _, policy)) =
                    self.parameters
                        .iter()
                        .rev()
                        .find(|(candidate, candidate_name, _)| {
                            (*candidate == symbol && symbol.is_valid())
                                || (!symbol.is_valid() && candidate_name == name.as_str())
                        })
                {
                    // An unbounded stored parameter may contain views. A
                    // suspension-allowing type-wide bound rules that out by
                    // the ordinary structural instantiation check; a value's
                    // Carry::AcrossSuspend qualification does not.
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
                // The empty product has no stored element, hence no policy
                // or loan demand. Nonempty and unresolved extents retain the
                // element's complete requirements without enumerating it.
                if length == typed_trees::types::FixedArrayLength::Literal(0) {
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
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {
                CarryPolicy::PERMISSIVE
            }
            // Borrows, slices, and erased satisfiers need per-value/provenance
            // evidence. Until that enforcement lands, absence fails closed.
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                // A descriptor's carry permissions cannot establish its
                // referent's lifetime, access restrictions or stable backing.
                // Keep this independent obligation through the same stored-
                // shape traversal, including concrete generic substitutions.
                self.requires_borrow_preservation = true;
                CarryPolicy::STRICT
            }
            TypeReferenceNode::DynamicTrait { .. } => {
                self.borrow_independent = self.borrow_independent.intersect(CarryPolicy::STRICT);
                CarryPolicy::STRICT
            }
        }
    }

    pub(super) fn derive_named_data(
        &mut self,
        symbol: symbols::SymbolHandle,
        name: &str,
        arguments: Option<&[typed_trees::types::TypeReferenceHandle]>,
    ) -> language_semantics::CarryPolicy {
        use language_semantics::CarryPolicy;

        let Some(definition) = self.program.data_definitions().iter().find(|definition| {
            (symbol.is_valid() && definition.symbol == symbol)
                || (!symbol.is_valid() && definition.name.as_str() == name)
        }) else {
            self.borrow_independent = self.borrow_independent.intersect(CarryPolicy::STRICT);
            return CarryPolicy::STRICT;
        };
        // Missing representation is not an empty record. This applies equally
        // to a direct carrier, a stored field and a generic argument.
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
                parameter.name.as_str().to_owned(),
                parameter.bounds.carry.unwrap_or(CarryPolicy::STRICT),
            ));
        }
        if let Some(arguments) = arguments {
            self.substitutions
                .extend(definition_parameters.iter().zip(arguments).map(
                    |(parameter, argument)| {
                        (
                            parameter.symbol,
                            parameter.name.as_str().to_owned(),
                            *argument,
                        )
                    },
                ));
        }

        let mut field_types = Vec::new();
        for_each_stored_field(self.program, definition, &mut |field, _| {
            field_types.push(field.type_reference);
        });
        // A declaration may deliberately impose a stricter floor than its
        // fields. Retain it at every named-data edge, including substitutions
        // and nested carriers; inspecting portable fields cannot erase the
        // declared CPU/thread, address or suspension obligation.
        let mut effective = definition
            .properties
            .carry
            .unwrap_or(CarryPolicy::PERMISSIVE);
        // All visited stored shapes contribute to the same meet. A view's
        // conservative fence is separate; explicit floors and unknown shapes
        // still contribute even when a descendant's loan can be preserved.
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

pub(super) fn for_each_stored_field(
    program: &dyn crate::CheckingInput,
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
