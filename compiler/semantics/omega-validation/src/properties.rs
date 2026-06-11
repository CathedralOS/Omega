//! Declared type-property verification (frozen decision 8).
//!
//! Properties are lowercase facts in brackets on a data declaration
//! (`data Point [copy, zero_init]`). The spelling set is closed at parse
//! time; this pass verifies the declared facts hold:
//!
//! - `copy`/`send`: structural — every field (and case payload field) must be
//!   a primitive or a data type that itself declares the property. Until the
//!   concurrency model lands, `send` uses the same structural walk as `copy`.
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
        if properties.send {
            validate_structural_property(program, symbols, data_definition, "send", diagnostics);
        }
        if properties.zero_init {
            validate_zero_init(program, symbols, data_definition, diagnostics);
        }
    }
}

/// `copy` and `send` are compositional: the property holds when every stored
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

/// One entry point for "does this type carry the declared property?", shared
/// with the instantiation-time bound check: `zero_init` has its own walk
/// (String's zeroed descriptor IS empty), `copy`/`send` share the structural
/// walk.
pub(crate) fn type_satisfies_declared_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
    property: &str,
) -> bool {
    if property == "zero_init" {
        type_is_zero_init(program, symbols, type_parameters, type_reference)
    } else {
        type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            type_reference,
            property,
        )
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
        return true;
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
pub(crate) fn declared_property_names(
    properties: &omega_typed_trees::data::DataProperties,
) -> Vec<&'static str> {
    let mut names = Vec::new();
    if properties.copy {
        names.push("copy");
    }
    if properties.zero_init {
        names.push("zero_init");
    }
    if properties.send {
        names.push("send");
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
        "send" => parameter.bounds.send,
        "zero_init" => parameter.bounds.zero_init,
        _ => false,
    }
}

/// The type parameter a field type ultimately names, if any — used to point
/// the diagnostic at the missing bound rather than the generic "not
/// `{property}`" wording.
fn referenced_type_parameter<'program>(
    program: &TypedTrees,
    type_parameters: &'program [TypeParameter],
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> Option<&'program TypeParameter> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            type_parameter_named(type_parameters, name.as_str())
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
            "send" => definition.properties.send,
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

        if field.initial_value.is_valid()
            && !expression_is_zero_literal(program, field.initial_value)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares `[zero_init]` but {place} has a non-zero default: zero-means-empty requires the zeroed value to be the empty value",
                data_definition.name
            )));
        }

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
        ExpressionNode::Integer(value) => *value == 0,
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
