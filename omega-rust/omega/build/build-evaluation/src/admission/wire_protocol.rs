//! Wire-protocol compatibility: every declared wire schema, era and
//! migration is validated against the build's compatibility demands, and
//! the report the compiler emits is built from the same walk.

use arena::HandleSpan;
use artifacts::{
    WireCaseReportEntry, WireCompatibilityDemandReportEntry, WireCompatibilityFactReport,
    WireCompatibilityVerdicts, WireFieldRelevance, WireFieldReportEntry, WireProtocolReport,
    WireRealizationOrigin, WireSchemaReportEntry, WireTrustClass, WireVersionReportEntry,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::wire::{WireMember, WireSchema};

pub fn validate_wire_protocol(
    typed: &TypedTrees,
    compatibility_demands: &[crate::WireCompatibilityDemand],
) -> Result<(), Vec<Diagnostic>> {
    validate_wire_protocol_report(&build_wire_protocol_report(typed, compatibility_demands))
}

fn validate_wire_protocol_report(report: &WireProtocolReport) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = report
        .demands
        .iter()
        .filter(|demand| !demand.satisfied)
        .map(|demand| {
            let failed = [
                ("readability", &demand.readability),
                ("writability", &demand.writability),
                ("unknown preservation", &demand.unknown_preservation),
                ("canonicality", &demand.canonicality),
                ("migration coverage", &demand.migration_coverage),
            ]
            .into_iter()
            .filter(|(_, fact)| fact.required && !fact.satisfied)
            .map(|(name, fact)| format!("{name} ({})", fact.detail))
            .collect::<Vec<_>>()
            .join(", ");
            Diagnostic::error(format!(
                "wire compatibility demand `{}` is unsatisfied for local schema `{}` and peer \
                 schema `{}`: {}",
                demand.edge, demand.local_schema, demand.peer_schema, failed
            ))
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// A published schema row paired with the qualified declaration path the
/// demand language selects it by. Era dispatch is the policy's authored path
/// resolving to one declaration; declaration order and leaf names alone do
/// not pick an era.
struct SchemaRow {
    qualified_path: String,
    entry: WireSchemaReportEntry,
    /// The runtime data definition a `PreservingDecode<Policy, Value>`
    /// implementation names as its value type, when the schema has one.
    /// `wire` declarations without a data counterpart carry none.
    value_symbol: Option<symbols::SymbolHandle>,
}

fn build_wire_protocol_report(
    typed: &TypedTrees,
    compatibility_demands: &[crate::WireCompatibilityDemand],
) -> WireProtocolReport {
    let mut rows = typed
        .wire_schemas()
        .iter()
        .map(|schema| SchemaRow {
            qualified_path: qualified_schema_path(typed, schema.symbol, schema.name.as_str()),
            entry: schema_report_entry(typed, schema),
            value_symbol: None,
        })
        .collect::<Vec<_>>();
    for (ordinary_path, ordinary, value_symbol) in
        typed.data_definitions().iter().filter_map(|data| {
            ordinary_data_schema_report_entry(typed, data).map(|entry| {
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
            generated.entry.normalized_schema_report_identity =
                ordinary.normalized_schema_report_identity;
            if generated.entry.fields.is_empty() {
                generated.entry.fields = ordinary.fields;
            }
            if generated.entry.reserved.is_empty() {
                generated.entry.reserved = ordinary.reserved;
            }
            generated.entry.cases = ordinary.cases;
            generated.entry.retired_cases = ordinary.retired_cases;
        } else {
            rows.push(SchemaRow {
                qualified_path: ordinary_path,
                entry: ordinary,
                value_symbol: Some(value_symbol),
            });
        }
    }
    for row in &mut rows {
        let schema = &mut row.entry;
        if !schema.synthesized_codec {
            continue;
        }
        let Some(source_schema) = typed.wire_schemas().iter().find(|candidate| {
            qualified_schema_path(typed, candidate.symbol, candidate.name.as_str())
                == row.qualified_path
        }) else {
            continue;
        };
        schema.codec_requirement = Some(format!("StrictDecode<compact_binary, {}>", schema.name));
        schema.codec_requirement_report_identity = Some(codec_requirement_report_identity(
            schema.normalized_schema_report_identity,
        ));
        schema.encode_requirement = Some(format!("Encode<compact_binary, {}>", schema.name));
        schema.encode_requirement_report_identity = Some(encode_requirement_report_identity(
            schema.normalized_schema_report_identity,
        ));
        schema.encode_obligations = typed
            .wire_schema_encode_obligations(source_schema.symbol)
            .unwrap_or_default()
            .iter()
            .map(|obligation| {
                format!(
                    "field {}: runtime element count; two scalar passes per element; remaining output capacity covers exact packed payload (element width {}, max varint bytes {})",
                    obligation.field_number,
                    obligation.element.byte_size,
                    obligation.element.max_varint_length()
                )
            })
            .collect();
        schema.normalized_plan_report_identity =
            typed
                .wire_schema_plan(source_schema.symbol)
                .map(|placements| {
                    normalized_wire_plan_report_identity(
                        schema.normalized_schema_report_identity,
                        placements,
                        typed
                            .wire_schema_encode_obligations(source_schema.symbol)
                            .unwrap_or_default(),
                    )
                });
        schema.realization_origin = Some(WireRealizationOrigin::Generated {
            generator: "Omega compiler compact_binary generator".to_owned(),
        });
        // Trust class follows the codec spec's realization table: the
        // generated body reports Derived only when its recorded plan carries
        // `policy_verified` — the authored `CompactBinary::plan` grammar
        // policy agreeing with the codec walk IS the independent check of
        // the public requirement. Absent the policy the realization stays
        // admitted under the compiler's authority.
        let policy_verified = typed.wire_schema_plan_policy_verified(source_schema.symbol);
        schema.trust_class = if policy_verified {
            Some(WireTrustClass::Derived)
        } else {
            Some(WireTrustClass::Admitted {
                authority: "Omega compiler".to_owned(),
            })
        };
        schema.realization_evidence = vec![
            "normalized compact_binary plan validated against the schema walk".to_owned(),
            if policy_verified {
                "generated codec plan independently checked against the authored \
                 `CompactBinary::plan` grammar policy; disagreement is a compile error"
                    .to_owned()
            } else {
                "generated body is not yet independently checked against the public codec \
                 requirement"
                    .to_owned()
            },
            "differential canaries are validation evidence, not derived-contract proof".to_owned(),
        ];
    }
    rows.sort_by(|left, right| left.entry.name.cmp(&right.entry.name));
    let demands = compatibility_demands
        .iter()
        .map(|demand| compatibility_demand_report(typed, &rows, demand))
        .collect();
    WireProtocolReport {
        schemas: rows.into_iter().map(|row| row.entry).collect(),
        demands,
    }
}

fn qualified_schema_path(
    typed: &TypedTrees,
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

fn codec_requirement_report_identity(schema_report_identity: u64) -> u64 {
    stable_wire_report_identity(
        b"omega.codec.requirement.v1",
        [
            b"StrictDecode".as_slice(),
            b"compact_binary".as_slice(),
            &schema_report_identity.to_le_bytes(),
        ],
    )
}

fn encode_requirement_report_identity(schema_report_identity: u64) -> u64 {
    stable_wire_report_identity(
        b"omega.encode.requirement.v1",
        [
            b"Encode".as_slice(),
            b"compact_binary".as_slice(),
            &schema_report_identity.to_le_bytes(),
        ],
    )
}

fn normalized_wire_plan_report_identity(
    schema_report_identity: u64,
    placements: &[typed_trees::wire::WirePlacement],
    obligations: &[typed_trees::wire::WireEncodeObligation],
) -> u64 {
    let mut parts = Vec::with_capacity(placements.len() + obligations.len() + 1);
    let schema_bytes = schema_report_identity.to_le_bytes();
    parts.push(schema_bytes.to_vec());
    for placement in placements {
        let (kind, tag) = match placement {
            typed_trees::wire::WirePlacement::Varint { tag } => (0u8, *tag),
            typed_trees::wire::WirePlacement::LengthPrefixed { tag } => (1u8, *tag),
        };
        let mut bytes = Vec::with_capacity(9);
        bytes.push(kind);
        bytes.extend_from_slice(&tag.to_le_bytes());
        parts.push(bytes);
    }
    for obligation in obligations {
        let mut bytes = Vec::with_capacity(20);
        bytes.push(2);
        bytes.extend_from_slice(&obligation.field_number.to_le_bytes());
        bytes.extend_from_slice(&(obligation.element.byte_size as u64).to_le_bytes());
        bytes.push(u8::from(obligation.element.zigzag));
        bytes.push(2); // two scalar passes per element
        bytes.push(1); // exact packed-payload capacity formula
        parts.push(bytes);
    }
    stable_wire_report_identity(b"omega.wire.plan.v1", parts.iter().map(Vec::as_slice))
}

fn stable_wire_report_identity<'a>(
    domain: &[u8],
    parts: impl IntoIterator<Item = &'a [u8]>,
) -> u64 {
    fn bytes(hash: &mut u64, value: &[u8]) {
        for byte in value {
            *hash ^= u64::from(*byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    let mut hash = 0xcbf29ce484222325u64;
    bytes(&mut hash, domain);
    for part in parts {
        bytes(&mut hash, &(part.len() as u64).to_le_bytes());
        bytes(&mut hash, part);
    }
    if hash == 0 { 1 } else { hash }
}

fn compatibility_demand_report(
    typed: &TypedTrees,
    rows: &[SchemaRow],
    demand: &crate::WireCompatibilityDemand,
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
enum EraSelection<T> {
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

fn select_era_path<'a, T>(
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
    demand: &crate::WireCompatibilityDemand,
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

fn name_leaf(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn schema_accepts(reader: &WireSchemaReportEntry, writer: &WireSchemaReportEntry) -> bool {
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

fn fields_equal(left: &[WireFieldReportEntry], right: &[WireFieldReportEntry]) -> bool {
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

/// Resolve an authored demand path to the exact ordinary data declaration it
/// names. Era identity is the declaration symbol: the same path rules as
/// schema dispatch apply, so a leaf must select one declaration unambiguously
/// rather than whichever era was declared first.
fn resolve_declared_era(typed: &TypedTrees, requested: &str) -> Option<symbols::SymbolHandle> {
    let candidates = typed
        .data_definitions()
        .iter()
        .map(|data| (typed.symbols.display_path(data.symbol, "::"), data.symbol))
        .collect::<Vec<_>>();
    match select_era_path(
        requested,
        candidates
            .iter()
            .map(|(path, symbol)| (path.as_str(), *symbol)),
    ) {
        EraSelection::Resolved(symbol) => Some(symbol),
        _ => None,
    }
}

/// The selected migration route: the bound machine names in peer-to-local
/// order for the report, plus the era declarations the route traverses,
/// oldest first, so retirement obligations can be evaluated along the exact
/// chain the policy selected.
struct MigrationRoute {
    machines: Vec<String>,
    eras: Vec<symbols::SymbolHandle>,
}

/// What the route search found between the selected eras. `Complete` carries
/// the uniquely bound checked conversion chain the report certifies;
/// `Ambiguous` means a complete chain exists but every one traverses an edge
/// bound by more than one machine, so no unique conversion is certified;
/// `Missing` means no bound chain exists at all.
enum MigrationRouteSearch {
    Complete(MigrationRoute),
    Ambiguous(String),
    Missing,
}

/// The checked migration route between two explicitly selected eras. Edges
/// are the machines bound to `FormatMigration<Lineage, Old, New>`; lineage,
/// old, and new are compared by declaration symbol, never by leaf name, so a
/// route bound on one module's declarations cannot satisfy another module's
/// demand even when the declarations are spelled identically. The certified
/// route may only traverse edges bound by exactly one machine: an edge bound
/// twice would certify whichever binding the search happened to reach first,
/// so such a route is reported ambiguous rather than selected.
fn migration_route(
    typed: &TypedTrees,
    lineage: &str,
    peer: &str,
    local: &str,
) -> MigrationRouteSearch {
    let (Some(local_symbol), Some(peer_symbol), Some(lineage_symbol)) = (
        resolve_declared_era(typed, local),
        resolve_declared_era(typed, peer),
        resolve_declared_era(typed, lineage),
    ) else {
        return MigrationRouteSearch::Missing;
    };
    if peer_symbol == local_symbol {
        return MigrationRouteSearch::Complete(MigrationRoute {
            machines: Vec::new(),
            eras: vec![local_symbol],
        });
    }
    let mut edges = Vec::new();
    for machine in typed.machines() {
        for conformance in typed.machine_trait_conformances(machine) {
            if name_leaf(conformance.name.as_str()) != "FormatMigration"
                || conformance.requirement.as_ref().map(|name| name.as_str()) != Some("migrate")
            {
                continue;
            }
            let arguments = typed
                .type_reference_table
                .type_reference_handles(conformance.arguments);
            if arguments.len() != 3 {
                continue;
            }
            let symbol = |index: usize| typed.type_reference_table.type_symbol(arguments[index]);
            if symbol(0) != lineage_symbol {
                continue;
            }
            let (old, new) = (symbol(1), symbol(2));
            if !old.is_valid() || !new.is_valid() {
                continue;
            }
            edges.push((old, new, machine.name.as_str().to_owned()));
        }
    }

    let uniquely_bound = |(old, new): &(symbols::SymbolHandle, symbols::SymbolHandle)| {
        edges
            .iter()
            .filter(|edge| (edge.0, edge.1) == (*old, *new))
            .count()
            == 1
    };
    let certified_edges = edges
        .iter()
        .filter(|(old, new, _)| uniquely_bound(&(*old, *new)))
        .cloned()
        .collect::<Vec<_>>();
    if let Some(route) = search_route(&certified_edges, peer_symbol, local_symbol) {
        return MigrationRouteSearch::Complete(route);
    }
    if let Some(route) = search_route(&edges, peer_symbol, local_symbol) {
        let hops = route
            .eras
            .windows(2)
            .filter_map(|hop| {
                let machines = edges
                    .iter()
                    .filter(|edge| (edge.0, edge.1) == (hop[0], hop[1]))
                    .map(|edge| format!("`{}`", edge.2))
                    .collect::<Vec<_>>();
                (machines.len() > 1).then(|| {
                    format!(
                        "edge `{}` -> `{}` is bound by {}",
                        typed.symbols.display_path(hop[0], "::"),
                        typed.symbols.display_path(hop[1], "::"),
                        machines.join(", ")
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("; ");
        return MigrationRouteSearch::Ambiguous(format!(
            "no uniquely bound `{lineage}` migration route exists from `{peer}` to `{local}`: \
             {hops}; bind exactly one machine to each checked conversion edge"
        ));
    }
    MigrationRouteSearch::Missing
}

/// Depth-first search for a bound edge chain from `peer` to `local` over the
/// given edges, returning the machines in peer-to-local order and the era
/// symbols the chain traverses, oldest first.
fn search_route(
    edges: &[(symbols::SymbolHandle, symbols::SymbolHandle, String)],
    peer: symbols::SymbolHandle,
    local: symbols::SymbolHandle,
) -> Option<MigrationRoute> {
    let mut frontier = vec![(peer, Vec::<String>::new(), vec![peer])];
    let mut visited = vec![peer];
    while let Some((current, machines, eras)) = frontier.pop() {
        for (old, new, machine) in edges {
            if *old != current {
                continue;
            }
            let mut next_machines = machines.clone();
            next_machines.push(machine.clone());
            let mut next_eras = eras.clone();
            next_eras.push(*new);
            if *new == local {
                return Some(MigrationRoute {
                    machines: next_machines,
                    eras: next_eras,
                });
            }
            if !visited.contains(new) {
                visited.push(*new);
                frontier.push((*new, next_machines, next_eras));
            }
        }
    }
    None
}

/// One numbering scope inside a published era, matching the parser's
/// declaration rules: record fields number independently of sum cases (mixed
/// data keeps both spaces and cannot tombstone), and each numbered case owns
/// a payload scope keyed by the case's stable identity.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IdentityScope {
    Fields,
    Cases,
    Payload(u64),
}

/// One era's live and retired stable identities inside one numbering scope.
struct NumberScope {
    key: IdentityScope,
    live: Vec<u64>,
    retired: Vec<u64>,
}

/// The numbered scopes one era publishes: a field scope, a case scope, and a
/// payload scope per numbered case. Every era emits both member scopes even
/// when one stays empty, so a shape change on the route still observes the
/// dropped identities as retirements. `retired #N;` joins the field scope
/// when the era declares no cases and the case scope when it declares no
/// fields -- the parser's own attribution, which covers both scopes for a
/// tombstone-only era. Returns `None` when the route's era is not an ordinary
/// data declaration -- such an era declares no stable identities, so it can
/// neither retire nor reuse one.
fn era_number_scopes(
    typed: &TypedTrees,
    era: symbols::SymbolHandle,
) -> Option<(String, Vec<NumberScope>)> {
    let definition = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == era)?;
    let members = typed.data_members(definition);
    let has_fields = members
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Field(_)));
    let has_cases = members
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)));
    let mut scopes = vec![
        NumberScope {
            key: IdentityScope::Fields,
            live: Vec::new(),
            retired: if has_cases {
                Vec::new()
            } else {
                definition.retired_identities.clone()
            },
        },
        NumberScope {
            key: IdentityScope::Cases,
            live: Vec::new(),
            retired: if has_fields {
                Vec::new()
            } else {
                definition.retired_identities.clone()
            },
        },
    ];
    for member in members {
        match member {
            typed_trees::data::DataMember::Field(field) => {
                if let Some(identity) = field.identity {
                    scopes[0].live.push(identity);
                }
            }
            typed_trees::data::DataMember::Variant(variant) => {
                if let Some(identity) = variant.identity {
                    scopes[1].live.push(identity);
                    scopes.push(NumberScope {
                        key: IdentityScope::Payload(identity),
                        live: typed
                            .data_payload_fields(variant)
                            .iter()
                            .filter_map(|field| field.identity)
                            .collect(),
                        retired: variant.retired_payload_identities.clone(),
                    });
                }
            }
        }
    }
    Some((
        qualified_schema_path(typed, era, definition.name.as_str()),
        scopes,
    ))
}

/// Retirement joins the selected route: an era retires a stable identity by
/// tombstoning it (`retired #N;`) or by dropping it from the published shape
/// the previous era on the route carried. Either way the identity is dead to
/// the lineage from that era on -- `FormatMigration` adds no era
/// discriminator to the payload, so a later era redeclaring the number would
/// decode stored old-era bytes under the new meaning. Returns the first reuse
/// found along the chain, or `None` when every era's identities are fresh.
fn retired_identity_reuse(typed: &TypedTrees, eras: &[symbols::SymbolHandle]) -> Option<String> {
    let mut retired: Vec<(IdentityScope, u64, String)> = Vec::new();
    let mut previous_live: Vec<(IdentityScope, Vec<u64>)> = Vec::new();
    for era in eras {
        let Some((era_name, scopes)) = era_number_scopes(typed, *era) else {
            continue;
        };
        for scope in &scopes {
            for identity in &scope.live {
                if let Some((_, _, retiring)) = retired
                    .iter()
                    .find(|(key, candidate, _)| *key == scope.key && candidate == identity)
                {
                    return Some(match scope.key {
                        IdentityScope::Fields => format!(
                            "era `{era_name}` redeclares stable identity #{identity} retired in era `{retiring}`"
                        ),
                        IdentityScope::Cases => format!(
                            "era `{era_name}` redeclares stable case identity #{identity} retired in era `{retiring}`"
                        ),
                        IdentityScope::Payload(case) => format!(
                            "era `{era_name}` redeclares stable payload identity #{identity} of case #{case} retired in era `{retiring}`"
                        ),
                    });
                }
            }
        }
        for scope in &scopes {
            for identity in &scope.retired {
                if !retired
                    .iter()
                    .any(|(key, candidate, _)| *key == scope.key && candidate == identity)
                {
                    retired.push((scope.key, *identity, era_name.clone()));
                }
            }
            if let Some((_, dropped)) = previous_live.iter().find(|(key, _)| *key == scope.key) {
                for identity in dropped {
                    if !scope.live.contains(identity)
                        && !retired
                            .iter()
                            .any(|(key, candidate, _)| *key == scope.key && candidate == identity)
                    {
                        retired.push((scope.key, *identity, era_name.clone()));
                    }
                }
            }
        }
        for scope in scopes {
            match previous_live.iter_mut().find(|(key, _)| *key == scope.key) {
                Some((_, live)) => *live = scope.live,
                None => previous_live.push((scope.key, scope.live)),
            }
        }
    }
    None
}

fn ordinary_data_schema_report_entry(
    typed: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
) -> Option<WireSchemaReportEntry> {
    use typed_trees::data::{DataMember, DataShapeKind};

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
            DataMember::Field(field) => Some(WireFieldReportEntry {
                number: field.identity?,
                name: field.name.to_string(),
                relevance: report_relevance(field.relevance),
                type_display: typed.display_type_reference(field.type_reference),
            }),
            DataMember::Variant(_) => None,
        })
        .collect();
    let cases = members
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(WireCaseReportEntry {
                number: variant.identity?,
                name: variant.name.to_string(),
                payload_fields: typed
                    .data_payload_fields(variant)
                    .iter()
                    .filter_map(|field| {
                        Some(WireFieldReportEntry {
                            number: field.identity?,
                            name: field.name.to_string(),
                            relevance: report_relevance(field.relevance),
                            type_display: typed.display_type_reference(field.type_reference),
                        })
                    })
                    .collect(),
                retired_payload_identities: variant.retired_payload_identities.clone(),
            }),
            DataMember::Field(_) => None,
        })
        .collect();
    let shape = typed_trees::data::DataDefinition::shape_kind_from_members(members);
    let (reserved, retired_cases) = match shape {
        DataShapeKind::Record => (data.retired_identities.clone(), Vec::new()),
        DataShapeKind::Enum => (Vec::new(), data.retired_identities.clone()),
        DataShapeKind::Empty | DataShapeKind::Mixed => (Vec::new(), Vec::new()),
    };
    Some(WireSchemaReportEntry {
        name: data.name.to_string(),
        normalized_schema_report_identity:
            build_time_evaluation::normalized_schema_report_fingerprint(typed, data),
        synthesized_codec: false,
        encoding: None,
        codec_requirement: None,
        codec_requirement_report_identity: None,
        encode_requirement: None,
        encode_requirement_report_identity: None,
        normalized_plan_report_identity: None,
        encode_obligations: Vec::new(),
        realization_origin: None,
        trust_class: None,
        realization_evidence: Vec::new(),
        current_era: 0,
        fields,
        reserved,
        cases,
        retired_cases,
        versions: Vec::new(),
    })
}

struct ScopeTable {
    fields: Vec<WireFieldReportEntry>,
    reserved: Vec<u64>,
}

fn collect_scope_table(typed: &TypedTrees, members: HandleSpan<WireMember>) -> ScopeTable {
    let mut table = ScopeTable {
        fields: Vec::new(),
        reserved: Vec::new(),
    };

    for member in typed.wire_members(members) {
        match member {
            WireMember::Field(field) => table.fields.push(WireFieldReportEntry {
                number: field.number,
                name: field.name.to_string(),
                relevance: report_relevance(field.relevance),
                type_display: typed.display_type_reference(field.type_reference),
            }),
            WireMember::Reserved(reserved) => table.reserved.push(reserved.number),
            WireMember::Version(_) => {}
        }
    }

    table
}

fn schema_report_entry(typed: &TypedTrees, schema: &WireSchema) -> WireSchemaReportEntry {
    let current = collect_scope_table(typed, schema.members);

    // The version chain, oldest declared era first; the current schema body is
    // the newest era. Each era's verdicts compare it against its SUCCESSOR in
    // the chain (v1 -> v2, ..., newest declared era -> current), matching how
    // decode migrations compose hop by hop.
    let mut eras: Vec<(String, ScopeTable)> = Vec::new();
    for member in typed.wire_members(schema.members) {
        let WireMember::Version(version) = member else {
            continue;
        };
        eras.push((
            version.name.to_string(),
            collect_scope_table(typed, version.members),
        ));
    }

    let mut versions = Vec::new();
    for index in 0..eras.len() {
        let (successor_name, successor_table) = match eras.get(index + 1) {
            Some((name, table)) => (name.clone(), table),
            None => ("current".to_owned(), &current),
        };
        let verdicts = compatibility_verdicts(&eras[index].1, successor_table);

        versions.push(WireVersionReportEntry {
            name: eras[index].0.clone(),
            // Decision 10 era assignment: declared version blocks count up
            // from era 0 in declaration order.
            era: index as u64,
            successor: successor_name,
            fields: std::mem::take(&mut eras[index].1.fields),
            reserved: std::mem::take(&mut eras[index].1.reserved),
            verdicts,
        });
    }

    WireSchemaReportEntry {
        name: schema.name.to_string(),
        normalized_schema_report_identity: 0,
        synthesized_codec: true,
        encoding: schema
            .encoding
            .as_ref()
            .map(|encoding| encoding.to_string()),
        codec_requirement: None,
        codec_requirement_report_identity: None,
        encode_requirement: None,
        encode_requirement_report_identity: None,
        normalized_plan_report_identity: None,
        encode_obligations: Vec::new(),
        realization_origin: None,
        trust_class: None,
        realization_evidence: Vec::new(),
        current_era: typed.wire_schema_current_era(schema),
        fields: current.fields,
        reserved: current.reserved,
        cases: Vec::new(),
        retired_cases: Vec::new(),
        versions,
    }
}

/// Mirrors the chapter 20 compatibility rules enforced in `validation`,
/// applied between an era and its SUCCESSOR in the version chain: stable or
/// renamed fields and additive fields are compatible; a stable field number
/// changing type across eras is legal evolution surfaced as "requires
/// migration" (the era discriminator selects the old era's decode table);
/// retired numbers must be reserved in the successor era. Incompatible
/// verdicts only appear here when validation also rejects the program; on a
/// passing build this section documents the evolution steps, including any
/// cross-era migrations a decoder must perform.
fn compatibility_verdicts(
    predecessor: &ScopeTable,
    successor: &ScopeTable,
) -> WireCompatibilityVerdicts {
    let mut verdicts = WireCompatibilityVerdicts::default();

    for field in &predecessor.fields {
        match successor
            .fields
            .iter()
            .find(|candidate| candidate.number == field.number)
        {
            Some(successor_field) => {
                if successor_field.relevance != field.relevance {
                    verdicts.requires_migration.push(format!(
                        "field {} {} changes relevance {} -> {}; the current codec placement changes and the old era must decode before migration",
                        field.number,
                        field.name,
                        report_relevance_name(field.relevance),
                        report_relevance_name(successor_field.relevance)
                    ));
                } else if successor_field.type_display != field.type_display {
                    verdicts.requires_migration.push(format!(
                        "field {} changes type {} -> {}; decode via the old era's table and migrate up the chain",
                        field.number, field.type_display, successor_field.type_display
                    ));
                } else if successor_field.name != field.name {
                    verdicts.compatible.push(format!(
                        "field {} renamed {} -> {} (number and type stable)",
                        field.number, field.name, successor_field.name
                    ));
                } else {
                    verdicts.compatible.push(format!(
                        "field {} {} {} unchanged",
                        field.number, field.name, field.type_display
                    ));
                }
            }
            None => {
                if successor.reserved.contains(&field.number) {
                    verdicts.reserved.push(format!(
                        "field {} {} retired; number reserved",
                        field.number, field.name
                    ));
                } else {
                    verdicts.incompatible.push(format!(
                        "field {} {} retired without reserving its number",
                        field.number, field.name
                    ));
                }
            }
        }
    }

    for field in &successor.fields {
        let existed = predecessor
            .fields
            .iter()
            .any(|candidate| candidate.number == field.number);

        if !existed {
            if field.relevance.is_erased() {
                verdicts.compatible.push(format!(
                    "added erased field {} {} {} (semantic identity only; no codec placement)",
                    field.number, field.name, field.type_display
                ));
            } else if predecessor.reserved.contains(&field.number) {
                verdicts.compatible.push(format!(
                    "added field {} {} {} (recycles a number the prior era retired; the era discriminator disambiguates)",
                    field.number, field.name, field.type_display
                ));
            } else {
                verdicts.compatible.push(format!(
                    "added field {} {} {}",
                    field.number, field.name, field.type_display
                ));
            }
        }
    }

    verdicts
}

fn report_relevance(relevance: language_core::BindingRelevance) -> WireFieldRelevance {
    match relevance {
        language_core::BindingRelevance::Relevant => WireFieldRelevance::Relevant,
        language_core::BindingRelevance::Erased => WireFieldRelevance::Erased,
    }
}

fn report_relevance_name(relevance: WireFieldRelevance) -> &'static str {
    match relevance {
        WireFieldRelevance::Relevant => "relevant",
        WireFieldRelevance::Erased => "erased",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ScopeTable, build_wire_protocol_report, codec_requirement_report_identity,
        compatibility_verdicts, encode_requirement_report_identity, fields_equal,
        normalized_wire_plan_report_identity, schema_accepts, search_route,
    };
    use artifacts::{WireFieldRelevance, WireFieldReportEntry, WireSchemaReportEntry};
    use typed_trees::wire::WirePlacement;

    fn field(
        number: u64,
        relevance: WireFieldRelevance,
        type_display: &str,
    ) -> WireFieldReportEntry {
        WireFieldReportEntry {
            number,
            name: format!("field_{number}"),
            relevance,
            type_display: type_display.to_owned(),
        }
    }

    #[test]
    fn directional_wire_shape_ignores_semantic_only_erased_fields() {
        let relevant = field(0, WireFieldRelevance::Relevant, "u32");
        let erased = field(1, WireFieldRelevance::Erased, "Evidence");

        assert!(fields_equal(
            &[relevant.clone(), erased],
            std::slice::from_ref(&relevant)
        ));
        assert!(!fields_equal(
            std::slice::from_ref(&relevant),
            &[field(0, WireFieldRelevance::Erased, "u32")]
        ));
    }

    #[test]
    fn relevance_change_requires_cross_era_migration() {
        let predecessor = ScopeTable {
            fields: vec![field(3, WireFieldRelevance::Relevant, "u32")],
            reserved: Vec::new(),
        };
        let successor = ScopeTable {
            fields: vec![field(3, WireFieldRelevance::Erased, "u32")],
            reserved: Vec::new(),
        };

        let verdicts = compatibility_verdicts(&predecessor, &successor);
        assert_eq!(verdicts.requires_migration.len(), 1);
        assert!(verdicts.requires_migration[0].contains("changes relevance relevant -> erased"));
        assert!(verdicts.compatible.is_empty());
    }

    #[test]
    fn codec_requirement_report_identity_binds_the_normalized_schema_report_coordinate() {
        assert_ne!(
            codec_requirement_report_identity(11),
            codec_requirement_report_identity(12)
        );
        assert_eq!(
            codec_requirement_report_identity(11),
            codec_requirement_report_identity(11)
        );
        assert_ne!(
            encode_requirement_report_identity(11),
            encode_requirement_report_identity(12)
        );
        assert_ne!(
            encode_requirement_report_identity(11),
            codec_requirement_report_identity(11),
            "encode and strict-decode are distinct requirement report coordinates"
        );
    }

    #[test]
    fn normalized_wire_plan_report_identity_binds_kind_tag_and_schema_report_coordinate() {
        let scalar = [WirePlacement::Varint { tag: 1 }];
        let length = [WirePlacement::LengthPrefixed { tag: 1 }];
        let retagged = [WirePlacement::Varint { tag: 2 }];

        let report_identity = normalized_wire_plan_report_identity(7, &scalar, &[]);
        assert_eq!(
            report_identity,
            normalized_wire_plan_report_identity(7, &scalar, &[])
        );
        assert_ne!(
            report_identity,
            normalized_wire_plan_report_identity(8, &scalar, &[])
        );
        assert_ne!(
            report_identity,
            normalized_wire_plan_report_identity(7, &length, &[])
        );
        assert_ne!(
            report_identity,
            normalized_wire_plan_report_identity(7, &retagged, &[])
        );

        let obligation = typed_trees::wire::WireEncodeObligation {
            field_number: 1,
            element: typed_trees::wire::WireScalarEncoding {
                byte_size: 4,
                zigzag: false,
            },
            length: typed_trees::wire::WireEncodeLengthObligation::RuntimeElementCount,
            work: typed_trees::wire::WireEncodeWorkObligation::TwoPassesPerElement,
            output_capacity:
                typed_trees::wire::WireEncodeOutputCapacityObligation::ExactPackedPayload,
        };
        assert_ne!(
            report_identity,
            normalized_wire_plan_report_identity(7, &scalar, &[obligation])
        );
    }

    #[test]
    fn compact_equal_wire_schema_reports_do_not_override_exact_shape_compatibility() {
        let reader = WireSchemaReportEntry {
            name: "Reader".to_owned(),
            normalized_schema_report_identity: 0xfeed,
            fields: vec![field(1, WireFieldRelevance::Relevant, "u32")],
            ..WireSchemaReportEntry::default()
        };
        let writer = WireSchemaReportEntry {
            name: "Writer".to_owned(),
            normalized_schema_report_identity: 0xfeed,
            fields: vec![field(1, WireFieldRelevance::Relevant, "Text")],
            ..WireSchemaReportEntry::default()
        };

        assert!(
            !schema_accepts(&reader, &writer),
            "compact-equal schema reports cannot authorize an incompatible exact wire shape"
        );
    }

    fn edge(
        old: u32,
        new: u32,
        machine: &str,
    ) -> (symbols::SymbolHandle, symbols::SymbolHandle, String) {
        (
            symbols::SymbolHandle::from_arena_index(old),
            symbols::SymbolHandle::from_arena_index(new),
            machine.to_owned(),
        )
    }

    #[test]
    fn checked_conversion_route_composes_eras_oldest_to_current() {
        // A two-era chain converts across an intermediate shape: the bound
        // machines run peer-to-local and the route records every era it
        // traverses, oldest first.
        let edges = [
            edge(1, 2, "V1::to_v2"),
            edge(2, 3, "V2::to_v3"),
            edge(9, 10, "Unrelated::edge"),
        ];

        let route = search_route(
            &edges,
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(3),
        )
        .expect("a bound edge chain reaches the local era");

        assert_eq!(route.machines, ["V1::to_v2", "V2::to_v3"]);
        assert_eq!(
            route.eras,
            [
                symbols::SymbolHandle::from_arena_index(1),
                symbols::SymbolHandle::from_arena_index(2),
                symbols::SymbolHandle::from_arena_index(3),
            ]
        );
    }

    #[test]
    fn checked_conversion_route_uses_a_direct_edge() {
        let edges = [edge(1, 2, "V1::to_v2")];

        let route = search_route(
            &edges,
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(2),
        )
        .expect("the single bound edge is the route");

        assert_eq!(route.machines, ["V1::to_v2"]);
        assert_eq!(route.eras.len(), 2);
    }

    #[test]
    fn checked_conversion_route_rejects_eras_with_no_bound_chain() {
        // A demand between eras that no FormatMigration edge connects must not
        // certify a conversion; partial chains that stop short do not satisfy
        // it either.
        let edges = [edge(1, 2, "V1::to_v2")];

        assert!(
            search_route(
                &edges,
                symbols::SymbolHandle::from_arena_index(1),
                symbols::SymbolHandle::from_arena_index(3),
            )
            .is_none()
        );
        assert!(
            search_route(
                &edges,
                symbols::SymbolHandle::from_arena_index(2),
                symbols::SymbolHandle::from_arena_index(1),
            )
            .is_none(),
            "a downgrade edge is a separate binding, not the reverse of the upgrade"
        );
    }

    #[test]
    fn checked_conversion_route_terminates_through_a_cycle_edge() {
        // Lineages may bind a downgrade edge alongside the upgrade; the
        // search must not follow the cycle back into a visited era.
        let cyclic = [edge(1, 2, "V1::to_v2"), edge(2, 1, "V2::to_v1")];
        assert!(
            search_route(
                &cyclic,
                symbols::SymbolHandle::from_arena_index(1),
                symbols::SymbolHandle::from_arena_index(3),
            )
            .is_none()
        );

        let cyclic_with_exit = [
            edge(1, 2, "V1::to_v2"),
            edge(2, 1, "V2::to_v1"),
            edge(2, 3, "V2::to_v3"),
        ];
        let route = search_route(
            &cyclic_with_exit,
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(3),
        )
        .expect("the route continues past the cycle to the local era");
        assert_eq!(route.machines, ["V1::to_v2", "V2::to_v3"]);
    }

    fn typed_fixture(source_text: &str) -> typed_trees::TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source_text)
            .tokenize()
            .expect("tokenize wire fixture");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse wire fixture");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve wire fixture");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type wire fixture")
    }

    fn wire_report(source_text: &str) -> artifacts::WireProtocolReport {
        let mut typed = typed_fixture(source_text);
        build_time_evaluation::compute_wire_plans(&mut typed, None, 0)
            .expect("wire plan pass accepts the fixture");
        build_wire_protocol_report(&typed, &[])
    }

    #[test]
    fn synthesized_codec_stays_admitted_without_an_authored_grammar_policy() {
        let report = wire_report(
            "data Packet { #1 seed: u64; #2 label: &[u8]; }\ndata Main { }\nmachine Main::main(&mut self) { }\n",
        );

        let packet = report
            .schemas
            .iter()
            .find(|schema| schema.name == "Packet")
            .expect("Packet schema row");
        assert!(packet.synthesized_codec);
        assert_eq!(
            packet.trust_class,
            Some(artifacts::WireTrustClass::Admitted {
                authority: "Omega compiler".to_owned()
            }),
            "a generated codec with no independent check remains compiler-admitted"
        );
        assert!(
            packet
                .realization_evidence
                .iter()
                .any(|line| line.contains("not yet independently checked"))
        );
    }

    #[test]
    fn policy_verified_generated_codec_reports_derived_trust() {
        // The authored `CompactBinary::plan` grammar policy agreeing with the
        // codec walk is the independent check of the public requirement; the
        // generated body then reports Derived (codec spec realization table).
        let report = wire_report(
            r#"
data Packet { #1 seed: u64; #2 label: &[u8]; }

data FieldKind { case Scalar; case Text; case Nested; case Repeated; }
data SchemaField { size: u64 [0..=4096]; align: u64 [1..=16]; number: i64; kind: FieldKind; }
data Schema { fields: [SchemaField; 32]; field_count: u64 [0..=32]; }
data FieldPlan [copy] { case Varint(tag: u64); case LengthPrefixed(tag: u64); }
data Plan { fields: [FieldPlan; 32]; entry_count: u64; size_fixed: u64; size_is_dynamic: bool; align: u64; }

data CompactBinary { fields: [FieldPlan; 32]; }
machine CompactBinary::plan(&mut self, schema: Schema) -> Plan {
    self.fields[0] = FieldPlan::Varint { tag: 1 };
    self.fields[1] = FieldPlan::LengthPrefixed { tag: 2 };
    Plan {
        fields: self.fields,
        entry_count: schema.field_count,
        size_fixed: 0,
        size_is_dynamic: true,
        align: 1,
    }
}

data Main { }
machine Main::main(&mut self) { }
"#,
        );

        let packet = report
            .schemas
            .iter()
            .find(|schema| schema.name == "Packet")
            .expect("Packet schema row");
        assert_eq!(
            packet.trust_class,
            Some(artifacts::WireTrustClass::Derived),
            "the policy-verified generated codec is independently checked, not admitted"
        );
        assert!(
            packet
                .realization_evidence
                .iter()
                .any(|line| line.contains("independently checked against the authored"))
        );
    }
}
