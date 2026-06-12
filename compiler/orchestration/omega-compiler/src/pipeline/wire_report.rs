use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, DataVersionReportEntry, WireCompatibilityVerdicts, WireFieldReportEntry,
    WireProtocolReport, WireSchemaReportEntry, WireVersionReportEntry,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::versioning;
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
    WireProtocolReport {
        schemas: typed
            .wire_schemas()
            .iter()
            .map(|schema| schema_report_entry(typed, schema))
            .collect(),
        data_versions: build_data_version_entries(typed),
    }
}

/// Versioned-data chain completeness (chapter 21, stage 3b): for every data
/// type with declared `version vN` blocks, report the era chain and whether
/// the prescribed migration machine (`T::from_vN`) exists for each era. A
/// missing migration is a VERDICT, never an error (frozen decision 14): a
/// `Versioned<T>` consumer may handle the old era manually in a version
/// match arm.
fn build_data_version_entries(typed: &TypedTrees) -> Vec<DataVersionReportEntry> {
    // Historical shapes (`Counter::v1`) sit right after their parent in
    // declaration order, so grouping by parent name preserves era order.
    let mut entries: Vec<DataVersionReportEntry> = Vec::new();
    for definition in typed.data_definitions() {
        let Some((data_name, version_name)) =
            versioning::split_version_shape_name(definition.name.as_str())
        else {
            continue;
        };

        match entries.iter_mut().find(|entry| entry.name == data_name) {
            Some(entry) => entry.versions.push(version_name.to_owned()),
            None => entries.push(DataVersionReportEntry {
                name: data_name.to_owned(),
                versions: vec![version_name.to_owned()],
                ..DataVersionReportEntry::default()
            }),
        }
    }

    for entry in &mut entries {
        entry.current_era = entry.versions.len() as u64;
        for version_name in &entry.versions {
            let machine_name = versioning::migration_machine_name(&entry.name, version_name);
            if typed
                .machines()
                .iter()
                .any(|machine| machine.name.as_str() == machine_name)
            {
                entry
                    .migrations
                    .push(format!("{version_name} -> current via `{machine_name}`"));
            } else {
                entry
                    .migrations
                    .push(format!("{version_name} -> current MISSING"));
                entry.missing.push(format!(
                    "era {version_name} declared but no `{machine_name}` exists; `Versioned<{}>` consumers must handle `{}::{version_name}` arms manually",
                    entry.name, entry.name
                ));
            }
        }
    }

    entries
}

struct ScopeTable {
    fields: Vec<WireFieldReportEntry>,
    reserved: Vec<i64>,
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
        encoding: schema
            .encoding
            .as_ref()
            .map(|encoding| encoding.to_string()),
        current_era: typed.wire_schema_current_era(schema),
        fields: current.fields,
        reserved: current.reserved,
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
