//! Declared type-property verification (frozen decision 8).
//!
//! Properties are lowercase facts in brackets on a data declaration
//! (`data Point [copy, zero_init]`). The spelling set is closed at parse
//! time; this pass verifies the declared facts hold:
//!
//! - `copy`: structural — every field (and case payload field) must be a
//!   primitive or a data type that itself declares the property.
//! - `carry(...)`: the authored four-axis floor may not be more permissive
//!   than the policy derived from every stored field.
//! - `zero_init` (zero means empty): the zero case must be payload-free, no
//!   field may declare a non-zero default, and nested data fields must
//!   themselves be `zero_init` so the zeroed aggregate is the empty value.
//!
//! Zero-VALIDITY (the all-zero pattern is a valid value) is the unconditional
//! layer-1 guarantee and needs no declaration; only zero-MEANS-EMPTY is
//! opt-in and verified here.

use crate::symbols::TopLevelSymbols;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::{DataDefinition, DataField, DataMember, TypeParameter};
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredPropertyRequirement {
    Copy,
    Linear,
    ZeroInit,
    Carry(omega_core::semantics::CarryPolicy),
}

impl std::fmt::Display for DeclaredPropertyRequirement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy => formatter.write_str("copy"),
            Self::Linear => formatter.write_str("linear"),
            Self::ZeroInit => formatter.write_str("zero_init"),
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

        if properties.copy {
            validate_structural_property(program, symbols, data_definition, "copy", diagnostics);
        }
        if let Some(carry) = properties.carry {
            validate_carry_policy(program, data_definition, carry, diagnostics);
        }
        if properties.zero_init {
            validate_zero_init(program, symbols, data_definition, diagnostics);
        }
        if properties.multiplicity != omega_core::semantics::Multiplicity::Linear {
            validate_no_linear_erasure(program, symbols, data_definition, diagnostics);
        }
    }
}

/// A stored linear obligation makes its enclosing value linear. V1 requires
/// that propagation to be explicit on the enclosing declaration so a field or
/// payload can never silently degrade into affine/drop-permitted ownership.
fn validate_no_linear_erasure(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    for_each_stored_field(program, data_definition, &mut |field, case: Option<&str>| {
        // A case payload is path-sensitive storage: `Empty | Live(Token)` is
        // allowed to remain an affine outer sum, with the linear obligation
        // present only while `Live` is active. Common/record fields have no
        // inactive case and therefore require unconditional propagation.
        if case.is_some() {
            return;
        }
        if !type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            field.type_reference,
            "linear",
        ) {
            return;
        }
        let place = match case {
            Some(case) => format!("case `{case}` payload field `{}`", field.name),
            None => format!("field `{}`", field.name),
        };
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` is affine but {place} carries a linear obligation; add `[linear]` to the enclosing data declaration so the obligation cannot be dropped",
            data_definition.name
        )));
    });
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
    for_each_stored_field(program, data_definition, &mut |field, case: Option<&str>| {
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
    });
}

fn validate_carry_policy(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    required: omega_core::semantics::CarryPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    for_each_stored_field(program, data_definition, &mut |field, case: Option<&str>| {
        let actual = derive_type_carry_policy(
            program,
            type_parameters,
            field.type_reference,
            &mut Vec::new(),
        );
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
    });
}

fn derive_type_carry_policy(
    program: &TypedTrees,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    visiting: &mut Vec<String>,
) -> omega_core::semantics::CarryPolicy {
    use omega_core::semantics::CarryPolicy;

    if program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some()
    {
        return CarryPolicy::PERMISSIVE;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            if let Some(parameter) = type_parameter_named(type_parameters, name.as_str()) {
                return parameter.bounds.carry.unwrap_or(CarryPolicy::STRICT);
            }
            named_type_carry_policy(program, name.as_str(), visiting)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            derive_type_carry_policy(program, type_parameters, *base_type, visiting)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            derive_type_carry_policy(program, type_parameters, *element_type, visiting)
        }
        TypeReferenceNode::Generic { base_name, .. } => {
            named_type_carry_policy(program, base_name.as_str(), visiting)
        }
        TypeReferenceNode::Unit => CarryPolicy::PERMISSIVE,
        // Borrows, slices, and erased satisfiers need per-value/provenance
        // evidence. Until that enforcement lands, absence fails closed.
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => CarryPolicy::STRICT,
    }
}

fn named_type_carry_policy(
    program: &TypedTrees,
    name: &str,
    visiting: &mut Vec<String>,
) -> omega_core::semantics::CarryPolicy {
    use omega_core::semantics::CarryPolicy;
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
    else {
        return CarryPolicy::STRICT;
    };
    if visiting.iter().any(|current| current == name) {
        return CarryPolicy::STRICT;
    }
    visiting.push(name.to_owned());
    let type_parameters = program.data_type_parameters(definition);
    let mut derived = CarryPolicy::PERMISSIVE;
    for_each_stored_field(program, definition, &mut |field, _| {
        derived = derived.intersect(derive_type_carry_policy(
            program,
            type_parameters,
            field.type_reference,
            visiting,
        ));
    });
    visiting.pop();
    derived
}

/// Derive the effective carry policy of a transparent data declaration from
/// its complete stored shape. This is the checker-owned result; an authored
/// `carry(...)` clause is only a minimum promise validated against it.
pub fn effective_data_carry_policy(
    program: &TypedTrees,
    data_definition: &DataDefinition,
) -> omega_core::semantics::CarryPolicy {
    use omega_core::semantics::CarryPolicy;

    let type_parameters = program.data_type_parameters(data_definition);
    let mut effective = CarryPolicy::PERMISSIVE;
    for_each_stored_field(program, data_definition, &mut |field, _| {
        effective = effective.intersect(derive_type_carry_policy(
            program,
            type_parameters,
            field.type_reference,
            &mut vec![data_definition.name.as_str().to_owned()],
        ));
    });
    effective
}

/// One entry point for "does this type carry the declared property?", shared
/// with the instantiation-time bound check: `zero_init` has its own walk and
/// carry requirements use the normalized four-axis comparison.
pub fn type_satisfies_declared_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    property: DeclaredPropertyRequirement,
) -> bool {
    match property {
        DeclaredPropertyRequirement::ZeroInit => {
            type_is_zero_init(program, symbols, type_parameters, type_reference)
        }
        DeclaredPropertyRequirement::Carry(required) => derive_type_carry_policy(
            program,
            type_parameters,
            type_reference,
            &mut Vec::new(),
        )
        .permits(required),
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
    if let Some(primitive) = program.type_reference_table.primitive_type(type_reference) {
        // String is lexed as a primitive but owns text storage: a bitwise copy
        // aliases the buffer, and crossing a spawn boundary moves ownership of
        // it. Scalars are the only copy-satisfying primitives.
        return property != "linear"
            && !matches!(primitive, omega_typed_trees::types::PrimitiveType::String);
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
    if properties.zero_init {
        names.push(DeclaredPropertyRequirement::ZeroInit);
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
        "linear" => {
            parameter.bounds.multiplicity == omega_core::semantics::Multiplicity::Linear
        }
        "zero_init" => parameter.bounds.zero_init,
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
                definition.properties.multiplicity
                    == omega_core::semantics::Multiplicity::Linear
            }
            "zero_init" => definition.properties.zero_init,
            _ => false,
        })
}

fn validate_zero_init(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    data_definition: &DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    let data_members = program.data_members(data_definition);

    // The zero case (tag 0) IS the empty value, so it cannot require payload
    // fields. This was a hard error for every case-bearing data before
    // properties landed; decision 8 demotes it into `[zero_init]`.
    let first_case = data_members.iter().find_map(|member| match member {
        DataMember::Variant(variant) => Some(variant),
        DataMember::Field(_) => None,
    });
    if let Some(variant) = first_case
        && variant.payload.count() > 0
    {
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` declares `[zero_init]` but zero case `{}` carries a payload: the first case is the zero-initialized empty value, so it must be payload-free",
            data_definition.name, variant.name
        )));
    }

    for_each_stored_field(program, data_definition, &mut |field, case: Option<&str>| {
        let place = match case {
            Some(case) => format!("case `{case}` payload field `{}`", field.name),
            None => format!("field `{}`", field.name),
        };


        if !type_is_zero_init(program, symbols, type_parameters, field.type_reference) {
            if let Some(parameter) =
                referenced_type_parameter(program, type_parameters, field.type_reference)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` declares `[zero_init]` but {place} type parameter `{name}` does not declare `[zero_init]` — add `{name} [zero_init]`",
                    data_definition.name,
                    name = parameter.name
                )));
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares `[zero_init]` but {place} is not zero-means-empty: nested data fields must declare `[zero_init]` themselves",
                data_definition.name
            )));
        }
    });
}

/// Primitives are zero-means-empty by definition (zero, 0.0, false, the zero
/// case); nested data must declare `[zero_init]` for the aggregate's zeroed
/// bytes to read as empty.
fn type_is_zero_init(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    if program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some()
    {
        return true;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            // A type parameter qualifies through its declared bound; the
            // owned String's zeroed descriptor IS its empty value, so allow
            // it explicitly; remaining named data must declare the property.
            if let Some(parameter) = type_parameter_named(type_parameters, name.as_str()) {
                return type_parameter_declares_property(parameter, "zero_init");
            }
            name.as_str() == "String"
                || named_type_declares_property(program, symbols, name.as_str(), "zero_init")
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_is_zero_init(program, symbols, type_parameters, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_is_zero_init(program, symbols, type_parameters, *element_type)
        }
        TypeReferenceNode::Generic { base_name, .. } => {
            named_type_declares_property(program, symbols, base_name.as_str(), "zero_init")
        }
        _ => false,
    }
}

fn expression_is_zero_literal(
    program: &TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_i64() == Some(0),
        ExpressionNode::Float(literal) => literal.value() == 0.0,
        ExpressionNode::Boolean(value) => !*value,
        _ => false,
    }
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
