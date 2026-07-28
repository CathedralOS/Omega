//! Declared type-property verification.
//!
//! Properties are lowercase facts in brackets on a data declaration
//! (`data Point [copy]`). The spelling set is closed at parse
//! time; this pass verifies the declared facts hold:
//!
//! - `copy`: structural — every field (and case payload field) must be a
//!   primitive or a data type that itself declares the property.
//! - `carry(...)`: the authored four-axis floor may not be more permissive
//!   than the policy derived from every stored field.
//! Whether zeroed storage establishes a type is derived from its default
//! domain, fields, and zero-case payload in `data`; semantic emptiness is not
//! a type property.

use crate::symbols::TopLevelSymbols;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataDefinition, DataField, DataMember, TypeParameter};
use omega_typed_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredPropertyRequirement {
    Copy,
    Linear,
    Carry(omega_core::semantics::CarryPolicy),
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

pub(crate) fn validate_data_properties(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        let properties = data_definition.properties;

        if data_definition.supply_mode == omega_core::semantics::DataSupplyMode::BoundaryOpaque {
            validate_opaque_data_properties(data_definition, diagnostics);
            continue;
        }

        if properties.copy {
            validate_structural_property(program, symbols, data_definition, "copy", diagnostics);
        }
        if let Some(carry) = properties.carry {
            validate_carry_policy(program, data_definition, carry, diagnostics);
        }
    }
}

/// `copy` is compositional: the property holds when every stored
/// field holds it. Primitives qualify; named data must declare the property
/// itself; a type parameter qualifies when it declares the matching bound
/// (`data Box<T [copy]> [copy]`, frozen decision 13); everything else
/// (references, slices, owned text, dyn traits) is rejected until a ruling
/// extends the set.
fn validate_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    data_definition: &DataDefinition,
    property: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    for_each_stored_field(
        program,
        data_definition,
        &mut |field, case: Option<&str>| {
            if type_satisfies_structural_property(
                program,
                symbols,
                type_parameters,
                field.type_reference,
                property,
            ) {
                return;
            }
            let place = match case {
                Some(case) => format!("case `{case}` payload field `{}`", field.name),
                None => format!("field `{}`", field.name),
            };
            if let Some(parameter) =
                referenced_type_parameter(program, type_parameters, field.type_reference)
            {
                diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares `[{property}]` but {place} type parameter `{name}` does not declare `[{property}]` — add `{name} [{property}]`",
                data_definition.name,
                name = parameter.name
            )));
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
            "data `{}` declares `[{property}]` but {place} is not `{property}`: only primitives and data types declaring `[{property}]` qualify",
            data_definition.name
        )));
        },
    );
}

fn validate_carry_policy(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    required: omega_core::semantics::CarryPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    for_each_stored_field(
        program,
        data_definition,
        &mut |field, case: Option<&str>| {
            let actual =
                CarryDerivation::new(program, type_parameters).derive(field.type_reference);
            if actual.permits(required) {
                return;
            }
            let place = match case {
                Some(case) => format!("case `{case}` payload field `{}`", field.name),
                None => format!("field `{}`", field.name),
            };
            if let Some(parameter) =
                referenced_type_parameter(program, type_parameters, field.type_reference)
            {
                diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares carry policy `{required}` but {place} type parameter `{}` has only `{actual}`; add a compatible `[carry(...)]` bound",
                data_definition.name, parameter.name
            )));
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares carry policy `{required}` but {place} permits only `{actual}`",
                data_definition.name
            )));
        },
    );
}

struct CarryDerivation<'program> {
    program: &'program TypedTrees,
    parameters: Vec<(
        omega_core::symbols::SymbolHandle,
        String,
        omega_core::semantics::CarryPolicy,
    )>,
    substitutions: Vec<(
        omega_core::symbols::SymbolHandle,
        String,
        omega_typed_trees::types::TypeReferenceHandle,
    )>,
    visiting: Vec<omega_core::symbols::SymbolHandle>,
}

impl<'program> CarryDerivation<'program> {
    fn new(program: &'program TypedTrees, parameters: &[TypeParameter]) -> Self {
        let parameters = parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    parameter
                        .bounds
                        .carry
                        .unwrap_or(omega_core::semantics::CarryPolicy::STRICT),
                )
            })
            .collect();
        Self {
            program,
            parameters,
            substitutions: Vec::new(),
            visiting: Vec::new(),
        }
    }

    fn derive(
        &mut self,
        type_reference: omega_typed_trees::types::TypeReferenceHandle,
    ) -> omega_core::semantics::CarryPolicy {
        use omega_core::semantics::CarryPolicy;

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
                    return *policy;
                }
                self.derive_named_data(symbol, name.as_str(), None)
            }
            TypeReferenceNode::Constrained { base_type, .. } => self.derive(base_type),
            TypeReferenceNode::FixedArray { element_type, .. } => self.derive(element_type),
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
            TypeReferenceNode::Unit => CarryPolicy::PERMISSIVE,
            // Borrows, slices, and erased satisfiers need per-value/provenance
            // evidence. Until that enforcement lands, absence fails closed.
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::DynamicTrait { .. } => CarryPolicy::STRICT,
        }
    }

    fn derive_named_data(
        &mut self,
        symbol: omega_core::symbols::SymbolHandle,
        name: &str,
        arguments: Option<&[omega_typed_trees::types::TypeReferenceHandle]>,
    ) -> omega_core::semantics::CarryPolicy {
        use omega_core::semantics::CarryPolicy;

        let Some(definition) = self.program.data_definitions().iter().find(|definition| {
            (symbol.is_valid() && definition.symbol == symbol)
                || (!symbol.is_valid() && definition.name.as_str() == name)
        }) else {
            return CarryPolicy::STRICT;
        };
        if self.visiting.contains(&definition.symbol) {
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
        let mut effective = CarryPolicy::PERMISSIVE;
        for field_type in field_types {
            effective = effective.intersect(self.derive(field_type));
        }

        self.substitutions.truncate(substitution_len);
        self.parameters.truncate(parameter_len);
        self.visiting.pop();
        effective
    }
}

/// Opaque declarations expose no representation the compiler could inspect.
/// Restrictive usage policy is safe (`[linear]`, or an explicitly strict carry
/// floor), while any claim that grants additional behavior must come from the
/// ordinary admission/receipt spine. Until that consumer lands, fail closed.
fn validate_opaque_data_properties(
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let properties = data_definition.properties;
    if properties.copy {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{}` cannot claim `[copy]` without an admitted property receipt",
            data_definition.name
        )));
    }
    if properties
        .carry
        .is_some_and(|policy| policy != omega_core::semantics::CarryPolicy::STRICT)
    {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{}` cannot relax its carry policy without an admitted property receipt",
            data_definition.name
        )));
    }
}

/// Derive the effective carry policy of a transparent data declaration from
/// its complete stored shape. This is the checker-owned result; an authored
/// `carry(...)` clause is only a minimum promise validated against it.
pub fn effective_data_carry_policy(
    program: &TypedTrees,
    data_definition: &DataDefinition,
) -> omega_core::semantics::CarryPolicy {
    if data_definition.supply_mode == omega_core::semantics::DataSupplyMode::BoundaryOpaque {
        return omega_core::semantics::CarryPolicy::STRICT;
    }
    CarryDerivation::new(program, &[]).derive_named_data(
        data_definition.symbol,
        data_definition.name.as_str(),
        None,
    )
}

/// Derive the effective carry policy of an arbitrary type at one generic
/// declaration site. Live-set checking consumes this entry point so it uses
/// the same structural derivation and concrete-argument substitution as
/// property validation; carry policy is never re-inferred from a type name.
pub fn effective_type_carry_policy(
    program: &TypedTrees,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> omega_core::semantics::CarryPolicy {
    CarryDerivation::new(program, type_parameters).derive(type_reference)
}

/// One entry point for "does this type carry the declared property?", shared
/// with the instantiation-time bound check. Carry requirements use the
/// normalized four-axis comparison.
pub fn type_satisfies_declared_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    property: DeclaredPropertyRequirement,
) -> bool {
    match property {
        DeclaredPropertyRequirement::Carry(required) => {
            CarryDerivation::new(program, type_parameters)
                .derive(type_reference)
                .permits(required)
        }
        DeclaredPropertyRequirement::Copy => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            type_reference,
            "copy",
        ),
        DeclaredPropertyRequirement::Linear => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            type_reference,
            "linear",
        ),
    }
}

fn type_satisfies_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    property: &str,
) -> bool {
    if program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some()
    {
        return property != "linear";
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            if let Some(parameter) = type_parameter_named(type_parameters, name.as_str()) {
                return type_parameter_declares_property(parameter, property);
            }
            named_type_declares_property(program, symbols, name.as_str(), property)
        }
        TypeReferenceNode::Constrained { base_type, .. } => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            *base_type,
            property,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            *element_type,
            property,
        ),
        // An instantiated generic carries the property when its base data
        // declares it; the instantiation-time bound check separately verifies
        // the arguments uphold the base's parameter bounds.
        TypeReferenceNode::Generic { base_name, .. } => {
            named_type_declares_property(program, symbols, base_name.as_str(), property)
        }
        // References, slices, dyn traits, unit: not part of the verified set
        // yet.
        _ => false,
    }
}

/// The declared property names in canonical order, for diagnostics and for
/// iterating a parameter's bounds.
pub fn declared_property_requirements(
    properties: &omega_typed_trees::data::DataProperties,
) -> Vec<DeclaredPropertyRequirement> {
    let mut names = Vec::new();
    if properties.copy {
        names.push(DeclaredPropertyRequirement::Copy);
    }
    if properties.multiplicity == omega_core::semantics::Multiplicity::Linear {
        names.push(DeclaredPropertyRequirement::Linear);
    }
    if let Some(carry) = properties.carry {
        names.push(DeclaredPropertyRequirement::Carry(carry));
    }
    names
}

fn type_parameter_named<'program>(
    type_parameters: &'program [TypeParameter],
    name: &str,
) -> Option<&'program TypeParameter> {
    type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
}

fn type_parameter_declares_property(parameter: &TypeParameter, property: &str) -> bool {
    match property {
        "copy" => parameter.bounds.copy,
        "linear" => parameter.bounds.multiplicity == omega_core::semantics::Multiplicity::Linear,
        _ => false,
    }
}

/// The type parameter a field or state-parameter type ultimately names, if
/// any — used to point the data-property diagnostic at the missing bound, and
/// by the machine-call bound check (decision 13) to find which parameter a
/// signature type like `&T` instantiates.
pub(crate) fn referenced_type_parameter<'program>(
    program: &TypedTrees,
    type_parameters: &'program [TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<&'program TypeParameter> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            type_parameter_named(type_parameters, name.as_str())
        }
        TypeReferenceNode::Reference { referee, .. } => {
            referenced_type_parameter(program, type_parameters, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            referenced_type_parameter(program, type_parameters, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            referenced_type_parameter(program, type_parameters, *element_type)
        }
        _ => None,
    }
}

fn named_type_declares_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    name: &str,
    property: &str,
) -> bool {
    // Builtin scalar spellings resolve as primitives above; a remaining named
    // type only qualifies when it is a data definition declaring the property.
    let _ = symbols;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .is_some_and(|definition| match property {
            "copy" => definition.properties.copy,
            "linear" => {
                definition.properties.multiplicity == omega_core::semantics::Multiplicity::Linear
            }
            _ => false,
        })
}

fn for_each_stored_field(
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
