use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, WireCaseReportEntry, WireCompatibilityVerdicts, WireFieldReportEntry,
    WireProtocolReport, WireSchemaReportEntry, WireVersionReportEntry,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::wire::{WireMember, WireSchema};

pub(super) fn write_wire_protocol_report(
    options: &CompileOptions,
    typed: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let report = build_wire_protocol_report(typed);

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_wire_protocol_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}

fn build_wire_protocol_report(typed: &TypedTrees) -> WireProtocolReport {
    let mut schemas = typed
        .wire_schemas()
        .iter()
        .map(|schema| schema_report_entry(typed, schema))
        .collect::<Vec<_>>();
    schemas.extend(
        typed
            .data_definitions()
            .iter()
            .filter_map(|data| ordinary_data_schema_report_entry(typed, data)),
    );
    schemas.sort_by(|left, right| left.name.cmp(&right.name));
    WireProtocolReport { schemas }
}

fn ordinary_data_schema_report_entry(
    typed: &TypedTrees,
    data: &omega_typed_trees::data::DataDefinition,
) -> Option<WireSchemaReportEntry> {
    use omega_typed_trees::data::{DataMember, DataShapeKind};

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
                            type_display: typed.display_type_reference(field.type_reference),
                        })
                    })
                    .collect(),
                retired_payload_identities: variant.retired_payload_identities.clone(),
            }),
            DataMember::Field(_) => None,
        })
        .collect();
    let shape = omega_typed_trees::data::DataDefinition::shape_kind_from_members(members);
    let (reserved, retired_cases) = match shape {
        DataShapeKind::Record => (data.retired_identities.clone(), Vec::new()),
        DataShapeKind::Enum => (Vec::new(), data.retired_identities.clone()),
        DataShapeKind::Empty | DataShapeKind::Mixed => (Vec::new(), Vec::new()),
    };
    Some(WireSchemaReportEntry {
        name: data.name.to_string(),
        normalized_schema_identity: super::layout_plans::normalized_schema_identity(typed, data),
        synthesized_codec: false,
        encoding: None,
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
        normalized_schema_identity: 0,
        synthesized_codec: true,
        encoding: schema
            .encoding
            .as_ref()
            .map(|encoding| encoding.to_string()),
        current_era: typed.wire_schema_current_era(schema),
        fields: current.fields,
        reserved: current.reserved,
        cases: Vec::new(),
        retired_cases: Vec::new(),
        versions,
    }
}

/// Mirrors the chapter 20 compatibility rules enforced in `omega-validation`,
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
                if successor_field.type_display != field.type_display {
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
            if predecessor.reserved.contains(&field.number) {
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
