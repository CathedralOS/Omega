//! Text rendering for the wire-protocol compatibility artifact.
//!
//! The compiler owns construction of the structured report. This module only
//! projects that retained structure into the stable human-readable artifact.

use psi_diagnostics::Diagnostic;

use super::{
    ArtifactWriter, WireCaseReportEntry, WireCompatibilityFactReport, WireFieldReportEntry,
    WireProtocolReport,
};

impl ArtifactWriter {
    pub fn write_wire_protocol_report(
        &self,
        wire_report: &WireProtocolReport,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Wire Protocols\n\n");
        output.push_str(&format!(
            "identity-keyed schemas: {}\n",
            wire_report.schemas.len()
        ));
        output.push_str(&format!(
            "edge compatibility demands: {}\n",
            wire_report.demands.len()
        ));

        for schema in &wire_report.schemas {
            output.push_str(&format!("\n## data {}\n", schema.name));
            output.push_str(&format!(
                "normalized schema identity: 0x{:016x}\n",
                schema.normalized_schema_identity
            ));
            output.push_str(&format!(
                "encoding: {}\n",
                schema
                    .encoding
                    .as_deref()
                    .unwrap_or("(selected by codec policy)")
            ));
            if schema.synthesized_codec {
                output.push_str(&format!("current era: {}\n", schema.current_era));
            }
            if let Some(requirement) = &schema.codec_requirement {
                output.push_str(&format!("codec requirement: {requirement}\n"));
            }
            if let Some(identity) = schema.codec_requirement_identity {
                output.push_str(&format!("codec requirement identity: 0x{identity:016x}\n"));
            }
            if let Some(requirement) = &schema.encode_requirement {
                output.push_str(&format!("encode requirement: {requirement}\n"));
            }
            if let Some(identity) = schema.encode_requirement_identity {
                output.push_str(&format!("encode requirement identity: 0x{identity:016x}\n"));
            }
            if let Some(identity) = schema.normalized_plan_identity {
                output.push_str(&format!("normalized plan identity: 0x{identity:016x}\n"));
            }
            if !schema.encode_obligations.is_empty() {
                output.push_str("encode obligations:\n");
                for obligation in &schema.encode_obligations {
                    output.push_str(&format!("  {obligation}\n"));
                }
            }
            if let Some(origin) = &schema.realization_origin {
                output.push_str(&format!("realization origin: {}\n", origin.describe()));
            }
            if let Some(trust) = &schema.trust_class {
                output.push_str(&format!("trust class: {}\n", trust.describe()));
            }
            if !schema.realization_evidence.is_empty() {
                output.push_str("realization evidence:\n");
                for evidence in &schema.realization_evidence {
                    output.push_str(&format!("  {evidence}\n"));
                }
            }

            // The generated codec surface: readable HERE, not only in a
            // validator's error strings.
            if schema.synthesized_codec {
                output.push_str("generated codec:\n");
                output.push_str(&format!(
                    "  machine {}::encode(&value, &mut out: [u8; N], &mut written: u64)\n",
                    schema.name
                ));
                output.push_str(&format!(
                    "  machine {}::decode(&mut value, &buffer: [u8; N], &mut read: u64, &mut verdict: WireVerdict)\n",
                    schema.name
                ));
            }

            push_wire_field_table(&mut output, &schema.fields, &schema.reserved);
            push_wire_case_table(&mut output, &schema.cases, &schema.retired_cases);

            for version in &schema.versions {
                output.push_str(&format!(
                    "\n### version {} (era {})\n",
                    version.name, version.era
                ));
                push_wire_field_table(&mut output, &version.fields, &version.reserved);

                output.push_str(&format!(
                    "\n### compatibility {} -> {}\n",
                    version.name, version.successor
                ));
                push_wire_verdicts(&mut output, "compatible", &version.verdicts.compatible);
                push_wire_verdicts(
                    &mut output,
                    "requires migration",
                    &version.verdicts.requires_migration,
                );
                push_wire_verdicts(&mut output, "reserved", &version.verdicts.reserved);
                push_wire_verdicts(&mut output, "incompatible", &version.verdicts.incompatible);
            }
        }

        for demand in &wire_report.demands {
            output.push_str(&format!("\n## compatibility demand {}\n", demand.edge));
            output.push_str(&format!("lineage: {}\n", demand.lineage));
            output.push_str(&format!("local schema: {}\n", demand.local_schema));
            output.push_str(&format!("peer schema: {}\n", demand.peer_schema));
            output.push_str(&format!("codec: {}\n", demand.codec));
            output.push_str(&format!(
                "unknown-member behavior: {}\n",
                demand.unknown_member_behavior
            ));
            push_wire_demand_fact(&mut output, "readability", &demand.readability);
            push_wire_demand_fact(&mut output, "writability", &demand.writability);
            push_wire_demand_fact(
                &mut output,
                "unknown preservation",
                &demand.unknown_preservation,
            );
            push_wire_demand_fact(&mut output, "canonicality", &demand.canonicality);
            push_wire_demand_fact(
                &mut output,
                "migration coverage",
                &demand.migration_coverage,
            );
            output.push_str(&format!(
                "verdict: {}\n",
                if demand.satisfied {
                    "satisfied"
                } else {
                    "unsatisfied"
                }
            ));
        }

        self.write_text("04_wire_protocols.txt", &output)
    }
}

fn push_wire_case_table(output: &mut String, cases: &[WireCaseReportEntry], retired: &[u64]) {
    output.push_str("cases:\n");
    if cases.is_empty() {
        output.push_str("  none\n");
    } else {
        for case in cases {
            output.push_str(&format!("  #{} {} payload:\n", case.number, case.name));
            if case.payload_fields.is_empty() {
                output.push_str("    none\n");
            } else {
                for field in &case.payload_fields {
                    output.push_str(&format!(
                        "    #{} {}{}: {}\n",
                        field.number,
                        field.name,
                        if field.relevance.is_erased() {
                            " [erased]"
                        } else {
                            ""
                        },
                        field.type_display
                    ));
                }
            }
            if !case.retired_payload_identities.is_empty() {
                output.push_str(&format!(
                    "    retired payload identities: {}\n",
                    case.retired_payload_identities
                        .iter()
                        .map(|identity| format!("#{identity}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }
    output.push_str("retired case identities: ");
    if retired.is_empty() {
        output.push_str("none\n");
    } else {
        output.push_str(
            retired
                .iter()
                .map(|identity| format!("#{identity}"))
                .collect::<Vec<_>>()
                .join(", ")
                .as_str(),
        );
        output.push('\n');
    }
}

fn push_wire_field_table(output: &mut String, fields: &[WireFieldReportEntry], reserved: &[u64]) {
    output.push_str("fields:\n");
    if fields.is_empty() {
        output.push_str("  none\n");
    } else {
        for field in fields {
            output.push_str(&format!(
                "  {} {}{} {}\n",
                field.number,
                field.name,
                if field.relevance.is_erased() {
                    " [erased]"
                } else {
                    ""
                },
                field.type_display
            ));
        }
    }

    if !reserved.is_empty() {
        output.push_str(&format!(
            "reserved: {}\n",
            reserved
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}

fn push_wire_verdicts(output: &mut String, label: &str, verdicts: &[String]) {
    output.push_str(&format!("{label}:\n"));
    if verdicts.is_empty() {
        output.push_str("  none\n");
    } else {
        for verdict in verdicts {
            output.push_str(&format!("  {verdict}\n"));
        }
    }
}

fn push_wire_demand_fact(output: &mut String, label: &str, fact: &WireCompatibilityFactReport) {
    output.push_str(&format!(
        "{label}: {} ({}) -- {}\n",
        if fact.satisfied { "yes" } else { "no" },
        if fact.required {
            "required"
        } else {
            "not required"
        },
        fact.detail
    ));
}

#[cfg(test)]
mod tests {
    use super::push_wire_field_table;
    use crate::{WireFieldRelevance, WireFieldReportEntry};

    #[test]
    fn wire_field_table_marks_erased_semantic_members() {
        let mut output = String::new();
        push_wire_field_table(
            &mut output,
            &[
                WireFieldReportEntry {
                    number: 1,
                    name: "value".to_owned(),
                    relevance: WireFieldRelevance::Relevant,
                    type_display: "u32".to_owned(),
                },
                WireFieldReportEntry {
                    number: 7,
                    name: "proof".to_owned(),
                    relevance: WireFieldRelevance::Erased,
                    type_display: "Evidence".to_owned(),
                },
            ],
            &[],
        );

        assert_eq!(
            output,
            "fields:\n  1 value u32\n  7 proof [erased] Evidence\n"
        );
    }
}
