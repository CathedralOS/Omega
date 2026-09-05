//! Exact replay for canonical structured-data const arguments.

use super::super::{exact_field_symbol, exact_top_level_data_symbol};
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbol_resolved_trees::{
    SymbolResolvedTrees,
    data::{DataDefinition, DataMember, DataShapeKind, DataVariant},
    types::{FixedArrayLength, TypeReference},
};
use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};

pub(super) fn carrier_is_supported(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> bool {
    let TypeReference::Named { symbol, name } = type_reference else {
        return false;
    };
    exact_named_type(source, *symbol, name.as_str())
        && source
            .data_definitions
            .iter()
            .find(|definition| definition.symbol == *symbol)
            .is_some_and(|definition| {
                data_carrier_is_supported(source, definition, &mut Vec::new())
            })
}

pub(super) fn declaration_is_supported(
    source: &SymbolResolvedTrees,
    declaration: &symbol_resolved_trees::constant::ConstDeclaration,
) -> bool {
    let Some(name_span) = source.symbols.symbol_source_span(declaration.symbol) else {
        return false;
    };
    declaration.symbol.is_valid()
        && source.symbols.get(declaration.symbol).kind == SymbolKind::Const
        && source.symbols.get(declaration.symbol).parent == source.symbols.root()
        && !source.symbols.name(declaration.symbol).is_empty()
        && name_span.source_id == declaration.initializer_source_span.source_id
        && !declaration.is_public
        && declaration.canonical_value_encoding.is_none()
        && carrier_is_supported(source, &declaration.declared_type)
}

pub(super) fn closed_argument_is_supported(
    source: &SymbolResolvedTrees,
    expected_type: &TypeReference,
    spelling: &str,
) -> bool {
    if !carrier_is_supported(source, expected_type) {
        return false;
    }
    let Some(value) = CanonicalConstValue::from_atom(spelling) else {
        return false;
    };
    let Some(expected_name) = type_reference_label(source, expected_type) else {
        return false;
    };
    if value.type_name != expected_name {
        return false;
    }
    let Some(decoded) = value.decode_encoding() else {
        return false;
    };
    value.display == canonical_display(&decoded)
        && decoded_value_matches(source, expected_type, &decoded, &mut Vec::new())
}

fn data_carrier_is_supported(
    source: &SymbolResolvedTrees,
    definition: &DataDefinition,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if visiting.contains(&definition.symbol)
        || !exact_top_level_data_symbol(source, definition)
        || definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape
        || definition.name.as_str() == "Rat"
        || !definition.lifetime_parameters.is_empty()
        || !definition.type_parameters.is_empty()
        || definition.generic_instance.is_some()
        || definition.quotient.is_some()
        || !definition.where_facts.is_empty()
        || definition.zero_gated
    {
        return false;
    }

    visiting.push(definition.symbol);
    let members = source.data_members(definition.members);
    let supported = match DataDefinition::shape_kind_from_members(members) {
        DataShapeKind::Record => members.iter().all(|member| {
            let DataMember::Field(field) = member else {
                return false;
            };
            !field.relevance.is_erased()
                && exact_field_symbol(source, definition.symbol, field)
                && type_carrier_is_supported(source, &field.type_reference, visiting)
        }),
        DataShapeKind::Enum => members.iter().all(|member| {
            let DataMember::Variant(variant) = member else {
                return false;
            };
            exact_variant_symbol(source, definition.symbol, variant)
                && source
                    .data_payload_fields(variant.payload)
                    .iter()
                    .all(|field| {
                        !field.relevance.is_erased()
                            && exact_field_symbol(source, variant.symbol, field)
                            && type_carrier_is_supported(source, &field.type_reference, visiting)
                    })
        }),
        DataShapeKind::Empty | DataShapeKind::Mixed => false,
    };
    visiting.pop();
    supported
}

fn type_carrier_is_supported(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    match type_reference {
        TypeReference::Named { symbol, name }
            if exact_named_type(source, *symbol, name.as_str()) =>
        {
            if scalar_atom(source, *symbol).is_some() {
                return true;
            }
            source
                .data_definitions
                .iter()
                .find(|definition| definition.symbol == *symbol)
                .is_some_and(|definition| data_carrier_is_supported(source, definition, visiting))
        }
        TypeReference::FixedArray(array) => {
            matches!(array.length, FixedArrayLength::Literal(_))
                && type_carrier_is_supported(
                    source,
                    source.child_type_reference(array.element_type),
                    visiting,
                )
        }
        TypeReference::Reference(_)
        | TypeReference::Constrained(_)
        | TypeReference::Slice(_)
        | TypeReference::Generic(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Named { .. }
        | TypeReference::Unit => false,
    }
}

fn decoded_value_matches(
    source: &SymbolResolvedTrees,
    expected_type: &TypeReference,
    value: &DecodedCanonicalConstValue,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    match expected_type {
        TypeReference::Named { symbol, name }
            if exact_named_type(source, *symbol, name.as_str()) =>
        {
            if let Some(atom) = scalar_atom(source, *symbol) {
                return scalar_value_matches(atom, value);
            }
            let Some(definition) = source
                .data_definitions
                .iter()
                .find(|definition| definition.symbol == *symbol)
            else {
                return false;
            };
            decoded_data_matches(source, definition, value, visiting)
        }
        TypeReference::FixedArray(array) => {
            let (
                FixedArrayLength::Literal(expected_length),
                DecodedCanonicalConstValue::Array { type_name, values },
            ) = (&array.length, value)
            else {
                return false;
            };
            type_reference_label(source, expected_type).as_deref() == Some(type_name.as_str())
                && values.len() == *expected_length
                && values.iter().all(|value| {
                    decoded_value_matches(
                        source,
                        source.child_type_reference(array.element_type),
                        value,
                        visiting,
                    )
                })
        }
        _ => false,
    }
}

fn decoded_data_matches(
    source: &SymbolResolvedTrees,
    definition: &DataDefinition,
    value: &DecodedCanonicalConstValue,
    visiting: &mut Vec<SymbolHandle>,
) -> bool {
    if visiting.contains(&definition.symbol)
        || !data_carrier_is_supported(source, definition, &mut Vec::new())
    {
        return false;
    }
    visiting.push(definition.symbol);
    let members = source.data_members(definition.members);
    let matches = match value {
        DecodedCanonicalConstValue::Record { type_name, fields }
            if type_name == definition.name.as_str()
                && DataDefinition::shape_kind_from_members(members) == DataShapeKind::Record
                && fields.len() == members.len() =>
        {
            members.iter().zip(fields).all(|(member, (name, value))| {
                let DataMember::Field(field) = member else {
                    return false;
                };
                name == field.name.as_str()
                    && decoded_value_matches(source, &field.type_reference, value, visiting)
            })
        }
        DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        } if type_name == definition.name.as_str()
            && DataDefinition::shape_kind_from_members(members) == DataShapeKind::Enum =>
        {
            let mut matching = members.iter().filter_map(|member| {
                let DataMember::Variant(variant) = member else {
                    return None;
                };
                (variant.name.as_str() == case_name).then_some(variant)
            });
            let Some(variant) = matching.next() else {
                visiting.pop();
                return false;
            };
            if matching.next().is_some() {
                visiting.pop();
                return false;
            }
            let payload = source.data_payload_fields(variant.payload);
            fields.len() == payload.len()
                && payload.iter().zip(fields).all(|(field, (name, value))| {
                    name == field.name.as_str()
                        && decoded_value_matches(source, &field.type_reference, value, visiting)
                })
        }
        _ => false,
    };
    visiting.pop();
    matches
}

fn scalar_value_matches(atom: BuiltinTypeAtom, value: &DecodedCanonicalConstValue) -> bool {
    match (atom, value) {
        (BuiltinTypeAtom::Bool, DecodedCanonicalConstValue::Boolean(_)) => true,
        (atom, DecodedCanonicalConstValue::Integer { type_name, value }) => {
            type_name == atom.identity() && super::const_arguments::integer_fits(atom, *value)
        }
        _ => false,
    }
}

fn scalar_atom(source: &SymbolResolvedTrees, symbol: SymbolHandle) -> Option<BuiltinTypeAtom> {
    source.symbols.builtin_type_atom(symbol).filter(|atom| {
        matches!(
            atom,
            BuiltinTypeAtom::Bool
                | BuiltinTypeAtom::I8
                | BuiltinTypeAtom::I16
                | BuiltinTypeAtom::I32
                | BuiltinTypeAtom::I64
                | BuiltinTypeAtom::U8
                | BuiltinTypeAtom::U16
                | BuiltinTypeAtom::U32
                | BuiltinTypeAtom::U64
                | BuiltinTypeAtom::Address
        )
    })
}

fn type_reference_label(
    source: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<String> {
    match type_reference {
        TypeReference::Named { symbol, name }
            if exact_named_type(source, *symbol, name.as_str()) =>
        {
            Some(name.as_str().to_owned())
        }
        TypeReference::FixedArray(array) => {
            let FixedArrayLength::Literal(length) = array.length else {
                return None;
            };
            Some(format!(
                "[{}; {length}]",
                type_reference_label(source, source.child_type_reference(array.element_type))?
            ))
        }
        _ => None,
    }
}

fn exact_named_type(source: &SymbolResolvedTrees, symbol: SymbolHandle, name: &str) -> bool {
    symbol.is_valid()
        && source.symbols.name(symbol) == name
        && matches!(
            source.symbols.get(symbol).kind,
            SymbolKind::BuiltinType | SymbolKind::Data
        )
}

fn exact_variant_symbol(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    variant: &DataVariant,
) -> bool {
    variant.symbol.is_valid()
        && source.symbols.get(variant.symbol).kind == SymbolKind::Variant
        && source.symbols.get(variant.symbol).parent == owner
        && source.symbols.name(variant.symbol) == variant.name.as_str()
}

fn canonical_display(value: &DecodedCanonicalConstValue) -> String {
    match value {
        DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
        DecodedCanonicalConstValue::Boolean(value) => value.to_string(),
        DecodedCanonicalConstValue::Array { values, .. } => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DecodedCanonicalConstValue::Record { type_name, fields } => format!(
            "{type_name} {{ {} }}",
            fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", canonical_display(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        } if fields.is_empty() => format!("{type_name}::{case_name}"),
        DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        } => format!(
            "{type_name}::{case_name} {{ {} }}",
            fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", canonical_display(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}
