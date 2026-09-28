//! Directional schema demands, explicit era selection and exact wire-shape comparison.

use super::migration::{MigrationRouteSearch, migration_route, retired_identity_reuse};
use super::schemas::{SchemaRow, qualified_schema_path};
use super::{
    WireCompatibilityDemand, WireCompatibilityDemandReportEntry, WireCompatibilityFactReport,
    WireFieldReportEntry, WireSchemaReportEntry,
};
use typed_trees::TypedTrees;

pub(super) fn compatibility_demand_report(
    typed: &TypedTrees,
    rows: &[SchemaRow],
    demand: &WireCompatibilityDemand,
) -> WireCompatibilityDemandReportEntry {
    let local = select_era_path(
        &demand.local_schema,
        rows.iter().map(|row| (row.qualified_path.as_str(), row)),
    );
    let peer = select_era_path(
        &demand.peer_schema,
        rows.iter().map(|row| (row.qualified_path.as_str(), row)),
    );
    let local_row = era_resolved(&local).copied();
    let peer_row = era_resolved(&peer).copied();
    let local_schema = local_row.map(|row| &row.entry);
    let peer_schema = peer_row.map(|row| &row.entry);
    let codec = local_schema
        .and_then(|schema| schema.encoding.as_deref())
        .or_else(|| peer_schema.and_then(|schema| schema.encoding.as_deref()))
        .unwrap_or("compact_binary")
        .to_owned();
    let compact_binary = codec == "compact_binary";

    let readability_value = local_schema
        .zip(peer_schema)
        .is_some_and(|(reader, writer)| schema_accepts(reader, writer));
    let writability_value = local_schema
        .zip(peer_schema)
        .is_some_and(|(writer, reader)| schema_accepts(reader, writer));
    let readable_detail = match (local_schema, peer_schema) {
        (Some(_), Some(_)) if readability_value => {
            "the local decoder accepts every peer shape".to_owned()
        }
        (Some(_), Some(_)) => {
            "the strict local decoder does not accept every peer shape".to_owned()
        }
        _ => schema_selection_detail(&local, &peer, demand),
    };
    let writable_detail = match (local_schema, peer_schema) {
        (Some(_), Some(_)) if writability_value => {
            "the peer decoder accepts every local shape".to_owned()
        }
        (Some(_), Some(_)) => {
            "the strict peer decoder does not accept every local shape".to_owned()
        }
        _ => schema_selection_detail(&local, &peer, demand),
    };

    let migration_route = migration_route(
        typed,
        &demand.lineage,
        &demand.peer_schema,
        &demand.local_schema,
    );
    let identity_reuse = match &migration_route {
        MigrationRouteSearch::Complete(route) => retired_identity_reuse(typed, &route.eras),
        _ => None,
    };
    let migration_value = local_schema.is_some()
        && peer_schema.is_some()
        && matches!(migration_route, MigrationRouteSearch::Complete(_))
        && identity_reuse.is_none();
    let migration_detail = match (local_schema, peer_schema, &migration_route) {
        (None, _, _) | (_, None, _) => schema_selection_detail(&local, &peer, demand),
        (Some(_), Some(_), MigrationRouteSearch::Missing) => {
            format!(
                "no complete `{}` migration route exists from `{}` to `{}`",
                demand.lineage, demand.peer_schema, demand.local_schema
            )
        }
        (Some(_), Some(_), MigrationRouteSearch::Ambiguous(detail)) => detail.clone(),
        (Some(_), Some(_), MigrationRouteSearch::Complete(route)) => {
            if let Some(reuse) = identity_reuse {
                format!("the selected route is not a sound migration: {reuse}")
            } else if route.machines.is_empty() {
                "peer and local schemas are identical; no migration edge is needed".to_owned()
            } else {
                format!("selected checked route: {}", route.machines.join(" -> "))
            }
        }
    };

    let readability = fact(demand.require_readable, readability_value, readable_detail);
    let writability = fact(demand.require_writable, writability_value, writable_detail);
    let preserving_decode = local_row
        .and_then(|row| row.value_symbol)
        .and_then(|symbol| published_preserving_decode(typed, symbol));
    let unknown_preservation = fact(
        demand.require_unknown_preservation,
        preserving_decode.is_some(),
        if let Some(detail) = preserving_decode {
            detail
        } else if compact_binary {
            "compact_binary publishes strict unknown-member behavior".to_owned()
        } else {
            format!("codec `{codec}` publishes no preserving behavior")
        },
    );
    let canonical_value = compact_binary && local_schema.is_some() && peer_schema.is_some();
    let canonical_detail = if local_schema.is_none() || peer_schema.is_none() {
        schema_selection_detail(&local, &peer, demand)
    } else if compact_binary {
        "compact_binary emits its canonical field order and scalar encodings".to_owned()
    } else {
        format!("codec `{codec}` publishes no canonicalization guarantee")
    };
    let canonicality = fact(demand.require_canonical, canonical_value, canonical_detail);
    let migration_coverage = fact(
        demand.require_complete_migration,
        migration_value,
        migration_detail,
    );
    let satisfied = [
        &readability,
        &writability,
        &unknown_preservation,
        &canonicality,
        &migration_coverage,
    ]
    .into_iter()
    .all(|fact| !fact.required || fact.satisfied);

    WireCompatibilityDemandReportEntry {
        edge: demand.edge.clone(),
        lineage: demand.lineage.clone(),
        local_schema: demand.local_schema.clone(),
        peer_schema: demand.peer_schema.clone(),
        codec,
        unknown_member_behavior: "strict".to_owned(),
        readability,
        writability,
        unknown_preservation,
        canonicality,
        migration_coverage,
        satisfied,
    }
}

/// Whether an authored `PreservingDecode<Policy, Value>` realization exists
/// for the local schema's value type — the codec publishing preserving
/// decode stops `PreserveUnknown` demands from being unsatisfiable. Returns
/// the report detail naming the realizing machine.
fn published_preserving_decode(
    typed: &TypedTrees,
    value_symbol: symbols::SymbolHandle,
) -> Option<String> {
    typed
        .machines()
        .iter()
        .flat_map(|machine| {
            typed
                .machine_trait_conformances(machine)
                .iter()
                .map(move |conformance| (machine, conformance))
        })
        .filter(|(machine, conformance)| {
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = typed_trees::machine::resolve_satisfied_declaration(
                typed, machine, conformance,
            )
            else {
                return false;
            };
            if definition.name.as_str() != "PreservingDecode"
                || requirement.name.as_str() != "decode_preserving"
            {
                return false;
            }
            typed
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .get(1)
                .is_some_and(|value| {
                    typed.type_reference_table.type_symbol(*value) == value_symbol
                })
        })
        .map(|(machine, _)| {
            format!(
                "codec publishes preserving decode: `{}` satisfies `PreservingDecode::decode_preserving` for the local schema's value type",
                qualified_schema_path(typed, machine.symbol, machine.name.as_str())
            )
        })
        .next()
}

fn fact(required: bool, satisfied: bool, detail: String) -> WireCompatibilityFactReport {
    WireCompatibilityFactReport {
        required,
        satisfied,
        detail,
    }
}

/// How the demand's authored path selects one era. An exact qualified-path
/// match wins; otherwise a bare or partial path must be the unambiguous
/// suffix of exactly one published declaration path. More than one candidate
/// is ambiguous — declaration order never picks an era — and none means the
/// policy named an era that is not published.
pub(super) enum EraSelection<T> {
    Resolved(T),
    Ambiguous(Vec<String>),
    Missing,
}

fn era_resolved<T>(selection: &EraSelection<T>) -> Option<&T> {
    match selection {
        EraSelection::Resolved(item) => Some(item),
        _ => None,
    }
}

pub(super) fn select_era_path<'a, T>(
    requested: &str,
    candidates: impl Iterator<Item = (&'a str, T)>,
) -> EraSelection<T> {
    let nested_suffix = format!("::{requested}");
    let mut exact = Vec::new();
    let mut nested = Vec::new();
    for (path, item) in candidates {
        if path == requested {
            exact.push((path, item));
        } else if path.ends_with(&nested_suffix) {
            nested.push((path, item));
        }
    }
    let (paths, items): (Vec<&str>, Vec<T>) = if exact.is_empty() {
        nested.into_iter().unzip()
    } else {
        exact.into_iter().unzip()
    };
    match items.len() {
        0 => EraSelection::Missing,
        1 => EraSelection::Resolved(items.into_iter().next().expect("one item")),
        _ => EraSelection::Ambiguous(
            paths
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<String>>(),
        ),
    }
}

fn schema_selection_detail<T>(
    local: &EraSelection<T>,
    peer: &EraSelection<T>,
    demand: &WireCompatibilityDemand,
) -> String {
    if matches!(local, EraSelection::Missing) && matches!(peer, EraSelection::Missing) {
        return format!(
            "neither local schema `{}` nor peer schema `{}` is published",
            demand.local_schema, demand.peer_schema
        );
    }
    [
        ("local", demand.local_schema.as_str(), local),
        ("peer", demand.peer_schema.as_str(), peer),
    ]
    .into_iter()
    .filter_map(|(role, authored, selection)| match selection {
        EraSelection::Resolved(_) => None,
        EraSelection::Missing => {
            Some(format!("{role} schema `{authored}` is not published"))
        }
        EraSelection::Ambiguous(candidates) => Some(format!(
            "{role} schema `{authored}` is ambiguous across published eras {}; qualify the era path in the demand",
            candidates
                .iter()
                .map(|candidate| format!("`{candidate}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    })
    .collect::<Vec<_>>()
    .join("; ")
}

pub(super) fn name_leaf(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

pub(super) fn schema_accepts(
    reader: &WireSchemaReportEntry,
    writer: &WireSchemaReportEntry,
) -> bool {
    if !reader.cases.is_empty() || !writer.cases.is_empty() {
        return writer.cases.iter().all(|writer_case| {
            reader.cases.iter().any(|reader_case| {
                reader_case.number == writer_case.number
                    && fields_equal(&reader_case.payload_fields, &writer_case.payload_fields)
            })
        });
    }
    fields_equal(&reader.fields, &writer.fields)
}

pub(super) fn fields_equal(left: &[WireFieldReportEntry], right: &[WireFieldReportEntry]) -> bool {
    let left_relevant = left
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .collect::<Vec<_>>();
    let right_relevant = right
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .collect::<Vec<_>>();
    left_relevant.len() == right_relevant.len()
        && left_relevant.iter().all(|left_field| {
            right_relevant.iter().any(|right_field| {
                left_field.number == right_field.number
                    && left_field.type_display == right_field.type_display
            })
        })
}
