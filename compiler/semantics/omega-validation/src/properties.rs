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
use omega_typed_trees::data::{DataDefinition, DataField, DataMember};
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
/// itself; everything else (references, slices, owned text, dyn traits,
/// generics, type parameters) is rejected until a ruling extends the set.
fn validate_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    data_definition: &DataDefinition,
    property: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for_each_stored_field(program, data_definition, &mut |field, case: Option<&str>| {
        if field_satisfies_structural_property(program, symbols, field, property) {
            return;
        }
        let place = match case {
            Some(case) => format!("case `{case}` payload field `{}`", field.name),
            None => format!("field `{}`", field.name),
        };
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` declares `[{property}]` but {place} is not `{property}`: only primitives and data types declaring `[{property}]` qualify",
            data_definition.name
        )));
    });
}

fn field_satisfies_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    field: &DataField,
    property: &str,
) -> bool {
    type_satisfies_structural_property(program, symbols, field.type_reference, property)
}

fn type_satisfies_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
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
        TypeReferenceNode::Named { name, .. } => named_type_declares_property(
            program,
            symbols,
            name.as_str(),
            property,
        ),
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_satisfies_structural_property(program, symbols, *base_type, property)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_satisfies_structural_property(program, symbols, *element_type, property)
        }
        // References, slices, generics, dyn traits, unit: not part of the
        // verified set yet (bound spelling for generics is an open ruling).
        _ => false,
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

        if !type_is_zero_init(program, symbols, field.type_reference) {
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
            // Non-data named types (String, type parameters, traits) are
            // outside the verified set; data must declare the property. The
            // owned String's zeroed descriptor IS its empty value, so allow it
            // explicitly.
            name.as_str() == "String"
                || named_type_declares_property(program, symbols, name.as_str(), "zero_init")
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_is_zero_init(program, symbols, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            type_is_zero_init(program, symbols, *element_type)
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
