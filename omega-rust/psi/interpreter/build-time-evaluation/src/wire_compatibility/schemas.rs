//! Publish exact schema rows and codec observations used by compatibility queries.

use super::demands::compatibility_demand_report;
use super::{
    WireCaseReportEntry, WireCompatibilityDemand, WireCompatibilityVerdicts, WireFieldRelevance,
    WireFieldReportEntry, WireProtocolReport, WireRealizationOrigin, WireSchemaReportEntry,
    WireTrustClass, WireVersionReportEntry,
};
use arena::HandleSpan;
use checked_interpreter::WireCodecVerification;
use std::collections::BTreeMap;
use typed_trees::TypedTrees;
use typed_trees::wire::{WireMember, WireSchema};

/// A published schema row paired with the qualified declaration path the
/// demand language selects it by. Era dispatch is the policy's authored path
/// resolving to one declaration; declaration order and leaf names alone do
/// not pick an era.
pub(super) struct SchemaRow {
    pub(super) qualified_path: String,
    pub(super) entry: WireSchemaReportEntry,
    /// The runtime data definition a `PreservingDecode<Policy, Value>`
    /// implementation names as its value type, when the schema has one.
    /// `wire` declarations without a data counterpart carry none.
    pub(super) value_symbol: Option<symbols::SymbolHandle>,
}

pub(super) fn build_wire_protocol_report(
    typed: &TypedTrees,
    compatibility_demands: &[WireCompatibilityDemand],
    verifications: &BTreeMap<String, Result<WireCodecVerification, String>>,
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
        // Trust class follows the codec spec's realization table: Derived is
        // "authored or generated body independently checked against the
        // public requirement" (spec, layouts/codecs.md). The spec requires
        // that evidence "establish the public requirement for every input
        // covered by its contract, tied to the exact codec realization and
        // schema".
        //
        // No current evidence tier reaches that bar for a generated codec:
        //
        // * The interpreter's generated-codec verification is a DIFFERENTIAL
        //   PROBE suite over selected members (Floor/Ceiling) and malformed
        //   frames. The spec is explicit: "differential probes over selected
        //   values and malformed frames are regression checks, not a
        //   general agreement argument. Reporting their finite scope does
        //   not qualify the realization as `Derived`."
        // * The authored `CompactBinary::plan` grammar policy is checked
        //   against the same obligation rather than standing in for it:
        //   `policy_verified` records that the policy's PLACEMENTS agree with
        //   the codec's walk (`build-time-evaluation/src/layouts/wire_plans.rs`),
        //   which is a check on the plan, not on the body's
        //   `decode(encode(value)) == value` for every input.
        //
        // Both are recorded as the evidence they are, and the realization
        // reports `Admitted`, naming the compiler as the trusted party,
        // until general agreement evidence tied to the exact realization
        // exists. A proven divergence never reports Derived: it is a
        // compile-time diagnostic.
        let plan_evidence =
            "normalized compact_binary plan validated against the schema walk".to_owned();
        let policy_verified = typed.wire_schema_plan_policy_verified(source_schema.symbol);
        let policy_evidence = || {
            "generated codec plan independently checked against the authored \
             `CompactBinary::plan` grammar policy; disagreement is a compile error"
                .to_owned()
        };
        match verifications.get(&row.qualified_path) {
            // The probe suite ran. Whether it closed with or without gaps,
            // its checks are finite-scope regression evidence: the codec
            // row stays compiler-admitted, and what was exercised (or left
            // uncovered) is recorded.
            Some(Ok(verification)) => {
                schema.trust_class = Some(WireTrustClass::Admitted {
                    authority: "Omega compiler".to_owned(),
                });
                // A schema the generator cannot realize at all exercises no
                // check; claiming "independent verification passed" over an
                // empty list would be evidence the run never produced.
                let mut evidence = vec![plan_evidence];
                if verification.checks.is_empty() {
                    evidence.push("independent verification exercised no check".to_owned());
                } else {
                    evidence.push(format!(
                        "independent verification passed: {}",
                        verification.checks.join("; ")
                    ));
                    if verification.gaps.is_empty() {
                        evidence.push(
                            "differential probes over selected members are regression \
                             checks, not the general agreement law; the realization \
                             stays compiler-admitted"
                                .to_owned(),
                        );
                    }
                }
                if policy_verified {
                    evidence.push(policy_evidence());
                }
                if !verification.gaps.is_empty() {
                    evidence.push(format!(
                        "independent verification coverage gap: {}",
                        verification.gaps.join("; ")
                    ));
                }
                schema.realization_evidence = evidence;
            }
            // A proven divergence: validation turns it into a diagnostic;
            // the row still records why the codec is untrusted.
            Some(Err(divergence)) => {
                schema.trust_class = Some(WireTrustClass::Admitted {
                    authority: "Omega compiler".to_owned(),
                });
                schema.realization_evidence = vec![
                    plan_evidence,
                    format!("independent verification found a codec divergence: {divergence}"),
                ];
            }
            None => {
                // No body check ran at all; plan agreement alone never
                // reaches Derived.
                schema.trust_class = Some(WireTrustClass::Admitted {
                    authority: "Omega compiler".to_owned(),
                });
                schema.realization_evidence = if policy_verified {
                    vec![
                        plan_evidence,
                        policy_evidence(),
                        "differential canaries are validation evidence, not derived-contract proof"
                            .to_owned(),
                    ]
                } else {
                    vec![
                        plan_evidence,
                        "generated body is not yet independently checked against the public codec \
                         requirement"
                            .to_owned(),
                        "differential canaries are validation evidence, not derived-contract proof"
                            .to_owned(),
                    ]
                };
            }
        }
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

pub(super) fn qualified_schema_path(
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

pub(super) fn codec_requirement_report_identity(schema_report_identity: u64) -> u64 {
    stable_wire_report_identity(
        b"omega.codec.requirement.v1",
        [
            b"StrictDecode".as_slice(),
            b"compact_binary".as_slice(),
            &schema_report_identity.to_le_bytes(),
        ],
    )
}

pub(super) fn encode_requirement_report_identity(schema_report_identity: u64) -> u64 {
    stable_wire_report_identity(
        b"omega.encode.requirement.v1",
        [
            b"Encode".as_slice(),
            b"compact_binary".as_slice(),
            &schema_report_identity.to_le_bytes(),
        ],
    )
}

pub(super) fn normalized_wire_plan_report_identity(
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
        normalized_schema_report_identity: crate::normalized_schema_report_fingerprint(typed, data),
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

pub(super) struct ScopeTable {
    pub(super) fields: Vec<WireFieldReportEntry>,
    pub(super) reserved: Vec<u64>,
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
pub(super) fn compatibility_verdicts(
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
