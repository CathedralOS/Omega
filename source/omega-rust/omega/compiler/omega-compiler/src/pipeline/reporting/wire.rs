use omega_artifacts::{
    WireCaseReportEntry, WireCompatibilityDemandReportEntry, WireCompatibilityFactReport,
    WireCompatibilityVerdicts, WireFieldRelevance, WireFieldReportEntry, WireProtocolReport,
    WireRealizationOrigin, WireSchemaReportEntry, WireTrustClass, WireVersionReportEntry,
};
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::wire::{WireMember, WireSchema};

pub(in crate::pipeline) fn validate_wire_protocol(
    typed: &TypedTrees,
    compatibility_demands: &[crate::pipeline::build_config::WireCompatibilityDemand],
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
            .map(|(name, _)| name)
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

fn build_wire_protocol_report(
    typed: &TypedTrees,
    compatibility_demands: &[crate::pipeline::build_config::WireCompatibilityDemand],
) -> WireProtocolReport {
    let mut schemas = typed
        .wire_schemas()
        .iter()
        .map(|schema| schema_report_entry(typed, schema))
        .collect::<Vec<_>>();
    for ordinary in typed
        .data_definitions()
        .iter()
        .filter_map(|data| ordinary_data_schema_report_entry(typed, data))
    {
        if let Some(generated) = schemas
            .iter_mut()
            .find(|schema| schema.name == ordinary.name)
        {
            generated.normalized_schema_report_identity =
                ordinary.normalized_schema_report_identity;
            if generated.fields.is_empty() {
                generated.fields = ordinary.fields;
            }
            if generated.reserved.is_empty() {
                generated.reserved = ordinary.reserved;
            }
            generated.cases = ordinary.cases;
            generated.retired_cases = ordinary.retired_cases;
        } else {
            schemas.push(ordinary);
        }
    }
    for schema in &mut schemas {
        if !schema.synthesized_codec {
            continue;
        }
        let Some(source_schema) = typed
            .wire_schemas()
            .iter()
            .find(|candidate| candidate.name.as_str() == schema.name)
        else {
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
        schema.trust_class = Some(WireTrustClass::Admitted {
            authority: "Omega compiler".to_owned(),
        });
        schema.realization_evidence = vec![
            "normalized compact_binary plan validated against the schema walk".to_owned(),
            "generated body is not yet independently checked against the public codec requirement"
                .to_owned(),
            "differential canaries are validation evidence, not derived-contract proof".to_owned(),
        ];
    }
    schemas.sort_by(|left, right| left.name.cmp(&right.name));
    let demands = compatibility_demands
        .iter()
        .map(|demand| compatibility_demand_report(typed, &schemas, demand))
        .collect();
    WireProtocolReport { schemas, demands }
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
    placements: &[psi_typed_trees::wire::WirePlacement],
    obligations: &[psi_typed_trees::wire::WireEncodeObligation],
) -> u64 {
    let mut parts = Vec::with_capacity(placements.len() + obligations.len() + 1);
    let schema_bytes = schema_report_identity.to_le_bytes();
    parts.push(schema_bytes.to_vec());
    for placement in placements {
        let (kind, tag) = match placement {
            psi_typed_trees::wire::WirePlacement::Varint { tag } => (0u8, *tag),
            psi_typed_trees::wire::WirePlacement::LengthPrefixed { tag } => (1u8, *tag),
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
    schemas: &[WireSchemaReportEntry],
    demand: &crate::pipeline::build_config::WireCompatibilityDemand,
) -> WireCompatibilityDemandReportEntry {
    let local = find_schema(schemas, &demand.local_schema);
    let peer = find_schema(schemas, &demand.peer_schema);
    let codec = local
        .and_then(|schema| schema.encoding.as_deref())
        .or_else(|| peer.and_then(|schema| schema.encoding.as_deref()))
        .unwrap_or("compact_binary")
        .to_owned();
    let compact_binary = codec == "compact_binary";

    let readability_value = local
        .zip(peer)
        .is_some_and(|(reader, writer)| schema_accepts(reader, writer));
    let writability_value = local
        .zip(peer)
        .is_some_and(|(writer, reader)| schema_accepts(reader, writer));
    let readable_detail = match (local, peer) {
        (Some(_), Some(_)) if readability_value => {
            "the local decoder accepts every peer shape".to_owned()
        }
        (Some(_), Some(_)) => {
            "the strict local decoder does not accept every peer shape".to_owned()
        }
        _ => missing_schema_detail(local, peer, demand),
    };
    let writable_detail = match (local, peer) {
        (Some(_), Some(_)) if writability_value => {
            "the peer decoder accepts every local shape".to_owned()
        }
        (Some(_), Some(_)) => {
            "the strict peer decoder does not accept every local shape".to_owned()
        }
        _ => missing_schema_detail(local, peer, demand),
    };

    let migration_route = migration_route(
        typed,
        &demand.lineage,
        &demand.peer_schema,
        &demand.local_schema,
    );
    let migration_value = local.is_some() && peer.is_some() && migration_route.is_some();
    let migration_detail = match (local, peer, migration_route) {
        (None, _, _) | (_, None, _) => missing_schema_detail(local, peer, demand),
        (Some(_), Some(_), None) => {
            format!(
                "no complete `{}` migration route exists from `{}` to `{}`",
                demand.lineage, demand.peer_schema, demand.local_schema
            )
        }
        (Some(_), Some(_), Some(route)) => {
            if route.is_empty() {
                "peer and local schemas are identical; no migration edge is needed".to_owned()
            } else {
                format!("selected checked route: {}", route.join(" -> "))
            }
        }
    };

    let readability = fact(demand.require_readable, readability_value, readable_detail);
    let writability = fact(demand.require_writable, writability_value, writable_detail);
    let unknown_preservation = fact(
        demand.require_unknown_preservation,
        false,
        if compact_binary {
            "compact_binary publishes strict unknown-member behavior".to_owned()
        } else {
            format!("codec `{codec}` publishes no preserving behavior")
        },
    );
    let canonical_value = compact_binary && local.is_some() && peer.is_some();
    let canonical_detail = if local.is_none() || peer.is_none() {
        missing_schema_detail(local, peer, demand)
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

fn fact(required: bool, satisfied: bool, detail: String) -> WireCompatibilityFactReport {
    WireCompatibilityFactReport {
        required,
        satisfied,
        detail,
    }
}

fn missing_schema_detail(
    local: Option<&WireSchemaReportEntry>,
    peer: Option<&WireSchemaReportEntry>,
    demand: &crate::pipeline::build_config::WireCompatibilityDemand,
) -> String {
    match (local, peer) {
        (None, None) => format!(
            "neither local schema `{}` nor peer schema `{}` is published",
            demand.local_schema, demand.peer_schema
        ),
        (None, Some(_)) => format!("local schema `{}` is not published", demand.local_schema),
        (Some(_), None) => format!("peer schema `{}` is not published", demand.peer_schema),
        (Some(_), Some(_)) => unreachable!("caller only asks for missing-schema details"),
    }
}

fn find_schema<'a>(
    schemas: &'a [WireSchemaReportEntry],
    requested: &str,
) -> Option<&'a WireSchemaReportEntry> {
    schemas
        .iter()
        .find(|schema| schema.name == requested)
        .or_else(|| {
            let requested_leaf = name_leaf(requested);
            let mut matching = schemas
                .iter()
                .filter(|schema| name_leaf(&schema.name) == requested_leaf);
            let only = matching.next()?;
            matching.next().is_none().then_some(only)
        })
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

fn migration_route(
    typed: &TypedTrees,
    lineage: &str,
    peer: &str,
    local: &str,
) -> Option<Vec<String>> {
    if names_match(peer, local) {
        return Some(Vec::new());
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
            let argument = |index: usize| typed.display_type_reference(arguments[index]);
            let edge_lineage = argument(0);
            if !names_match(&edge_lineage, lineage) {
                continue;
            }
            edges.push((argument(1), argument(2), machine.name.as_str().to_owned()));
        }
    }

    let mut frontier = vec![(peer.to_owned(), Vec::<String>::new())];
    let mut visited = vec![peer.to_owned()];
    while let Some((current, route)) = frontier.pop() {
        for (old, new, machine) in &edges {
            if !names_match(old, &current) {
                continue;
            }
            let mut next_route = route.clone();
            next_route.push(machine.clone());
            if names_match(new, local) {
                return Some(next_route);
            }
            if !visited.iter().any(|seen| names_match(seen, new)) {
                visited.push(new.clone());
                frontier.push((new.clone(), next_route));
            }
        }
    }
    None
}

fn names_match(left: &str, right: &str) -> bool {
    left == right || name_leaf(left) == name_leaf(right)
}

fn ordinary_data_schema_report_entry(
    typed: &TypedTrees,
    data: &psi_typed_trees::data::DataDefinition,
) -> Option<WireSchemaReportEntry> {
    use psi_typed_trees::data::{DataMember, DataShapeKind};

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
    let shape = psi_typed_trees::data::DataDefinition::shape_kind_from_members(members);
    let (reserved, retired_cases) = match shape {
        DataShapeKind::Record => (data.retired_identities.clone(), Vec::new()),
        DataShapeKind::Enum => (Vec::new(), data.retired_identities.clone()),
        DataShapeKind::Empty | DataShapeKind::Mixed => (Vec::new(), Vec::new()),
    };
    Some(WireSchemaReportEntry {
        name: data.name.to_string(),
        normalized_schema_report_identity:
            psi_build_time_evaluation::normalized_schema_report_fingerprint(typed, data),
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

/// Mirrors the chapter 20 compatibility rules enforced in `psi-validation`,
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

fn report_relevance(relevance: psi_language_core::BindingRelevance) -> WireFieldRelevance {
    match relevance {
        psi_language_core::BindingRelevance::Relevant => WireFieldRelevance::Relevant,
        psi_language_core::BindingRelevance::Erased => WireFieldRelevance::Erased,
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
        ScopeTable, codec_requirement_report_identity, compatibility_verdicts,
        encode_requirement_report_identity, fields_equal, normalized_wire_plan_report_identity,
        schema_accepts,
    };
    use omega_artifacts::{WireFieldRelevance, WireFieldReportEntry, WireSchemaReportEntry};
    use psi_typed_trees::wire::WirePlacement;

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

        let obligation = psi_typed_trees::wire::WireEncodeObligation {
            field_number: 1,
            element: psi_typed_trees::wire::WireScalarEncoding {
                byte_size: 4,
                zigzag: false,
            },
            length: psi_typed_trees::wire::WireEncodeLengthObligation::RuntimeElementCount,
            work: psi_typed_trees::wire::WireEncodeWorkObligation::TwoPassesPerElement,
            output_capacity:
                psi_typed_trees::wire::WireEncodeOutputCapacityObligation::ExactPackedPayload,
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
}
