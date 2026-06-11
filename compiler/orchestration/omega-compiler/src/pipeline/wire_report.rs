use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, WireCompatibilityVerdicts, WireFieldReportEntry, WireProtocolReport,
    WireSchemaReportEntry, WireVersionReportEntry,
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
    WireProtocolReport {
        schemas: typed
            .wire_schemas()
            .iter()
            .map(|schema| schema_report_entry(typed, schema))
            .collect(),
    }
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
    let mut versions = Vec::new();

    for member in typed.wire_members(schema.members) {
        let WireMember::Version(version) = member else {
            continue;
        };
        let version_table = collect_scope_table(typed, version.members);
        let verdicts = compatibility_verdicts(&version_table, &current);

        versions.push(WireVersionReportEntry {
            name: version.name.to_string(),
            fields: version_table.fields,
            reserved: version_table.reserved,
            verdicts,
        });
    }

    WireSchemaReportEntry {
        name: schema.name.to_string(),
        encoding: schema
            .encoding
            .as_ref()
            .map(|encoding| encoding.to_string()),
        fields: current.fields,
        reserved: current.reserved,
        versions,
    }
}

/// Mirrors the chapter 20 compatibility rules enforced in `omega-validation`:
/// stable or renamed fields and additive fields are compatible, retired
/// numbers must be reserved, and type changes without a decode rule are
/// incompatible. Incompatible verdicts only appear here when validation also
/// rejects the program; on a passing build this section documents the safe
/// evolution steps.
fn compatibility_verdicts(
    version: &ScopeTable,
    current: &ScopeTable,
) -> WireCompatibilityVerdicts {
    let mut verdicts = WireCompatibilityVerdicts::default();

    for field in &version.fields {
        match current
            .fields
            .iter()
            .find(|candidate| candidate.number == field.number)
        {
            Some(current_field) => {
                if current_field.type_display != field.type_display {
                    verdicts.incompatible.push(format!(
                        "field {} changed {} -> {} without decode rule",
                        field.number, field.type_display, current_field.type_display
                    ));
                } else if current_field.name != field.name {
                    verdicts.compatible.push(format!(
                        "field {} renamed {} -> {} (number and type stable)",
                        field.number, field.name, current_field.name
                    ));
                } else {
                    verdicts.compatible.push(format!(
                        "field {} {} {} unchanged",
                        field.number, field.name, field.type_display
                    ));
                }
            }
            None => {
                if current.reserved.contains(&field.number) {
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

    for field in &current.fields {
        let existed = version
            .fields
            .iter()
            .any(|candidate| candidate.number == field.number);

        if !existed {
            verdicts.compatible.push(format!(
                "added field {} {} {}",
                field.number, field.name, field.type_display
            ));
        }
    }

    verdicts
}
