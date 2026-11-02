//! Project exact wire shapes for directional compatibility checking.

use super::demands::compatibility_demand_verdict;
use super::{WireCase, WireCompatibilityDemand, WireDemandVerdict, WireField, WireShape};
use crate::CheckingInput;
use arena::HandleSpan;
use typed_trees::data::DataDefinition;
use typed_trees::wire::{WireMember, WireSchema};

/// Exact schema path and the value type used to select PreservingDecode.
pub(super) struct SchemaRow {
    pub(super) qualified_path: String,
    pub(super) entry: WireShape,
    pub(super) value_symbol: Option<symbols::SymbolHandle>,
}

pub(super) fn check_wire_demands(
    typed: &dyn CheckingInput,
    compatibility_demands: &[WireCompatibilityDemand],
) -> Vec<WireDemandVerdict> {
    if compatibility_demands.is_empty() {
        return Vec::new();
    }
    let mut rows = typed
        .wire_schemas()
        .iter()
        .map(|schema| SchemaRow {
            qualified_path: qualified_schema_path(typed, schema.symbol, schema.name.as_str()),
            entry: schema_shape(typed, schema),
            value_symbol: None,
        })
        .collect::<Vec<_>>();
    for (ordinary_path, ordinary, value_symbol) in
        typed.data_definitions().iter().filter_map(|data| {
            ordinary_data_shape(typed, data).map(|entry| {
                (
                    qualified_schema_path(typed, data.symbol, data.name.as_str()),
                    entry,
                    data.symbol,
                )
            })
        })
    {
        if let Some(generated) = rows
            .iter_mut()
            .find(|row| row.qualified_path == ordinary_path)
        {
            generated.value_symbol = Some(value_symbol);
            if generated.entry.fields.is_empty() {
                generated.entry.fields = ordinary.fields;
            }
            generated.entry.cases = ordinary.cases;
        } else {
            rows.push(SchemaRow {
                qualified_path: ordinary_path,
                entry: ordinary,
                value_symbol: Some(value_symbol),
            });
        }
    }
    rows.sort_by(|left, right| left.entry.name.cmp(&right.entry.name));
    compatibility_demands
        .iter()
        .map(|demand| compatibility_demand_verdict(typed, &rows, demand))
        .collect()
}

pub(super) fn qualified_schema_path(
    typed: &dyn CheckingInput,
    symbol: symbols::SymbolHandle,
    fallback_name: &str,
) -> String {
    let path = typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        fallback_name.to_owned()
    } else {
        path
    }
}

fn ordinary_data_shape(typed: &dyn CheckingInput, data: &DataDefinition) -> Option<WireShape> {
    use typed_trees::data::DataMember;

    let members = typed.data_members(data);
    let has_identity_metadata = !data.retired_identities.is_empty()
        || members.iter().any(|member| match member {
            DataMember::Field(field) => field.identity.is_some(),
            DataMember::Variant(variant) => {
                variant.identity.is_some()
                    || !variant.retired_payload_identities.is_empty()
                    || typed
                        .data_payload_fields(variant)
                        .iter()
                        .any(|field| field.identity.is_some())
            }
        });
    if !has_identity_metadata {
        return None;
    }
    let fields = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(WireField {
                number: field.identity?,
                relevance: field.relevance,
                type_display: typed.display_type_reference(field.type_reference),
            }),
            DataMember::Variant(_) => None,
        })
        .collect();
    let cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(WireCase {
                number: variant.identity?,
                payload_fields: typed
                    .data_payload_fields(variant)
                    .iter()
                    .filter_map(|field| {
                        Some(WireField {
                            number: field.identity?,
                            relevance: field.relevance,
                            type_display: typed.display_type_reference(field.type_reference),
                        })
                    })
                    .collect(),
            }),
            DataMember::Field(_) => None,
        })
        .collect();
    Some(WireShape {
        name: data.name.to_string(),
        encoding: None,
        fields,
        cases,
    })
}

fn collect_fields(typed: &dyn CheckingInput, members: HandleSpan<WireMember>) -> Vec<WireField> {
    typed
        .wire_members(members)
        .iter()
        .filter_map(|member| match member {
            WireMember::Field(field) => Some(WireField {
                number: field.number,
                relevance: field.relevance,
                type_display: typed.display_type_reference(field.type_reference),
            }),
            WireMember::Reserved(_) | WireMember::Version(_) => None,
        })
        .collect()
}

fn schema_shape(typed: &dyn CheckingInput, schema: &WireSchema) -> WireShape {
    WireShape {
        name: schema.name.to_string(),
        encoding: schema
            .encoding
            .as_ref()
            .map(|encoding| encoding.to_string()),
        fields: collect_fields(typed, schema.members),
        cases: Vec::new(),
    }
}
