//! Chapter-10 trust and provider-qualification artifact presentation.

use diagnostics::Diagnostic;

use super::{ArtifactWriter, TrustReport, hex_bytes};

impl TrustReport {
    /// Validate exact target/report joins independently from filesystem output.
    /// Observation suppression must not suppress these consistency checks.
    pub fn validate(&self) -> Result<(), Diagnostic> {
        for row in &self.provider_requirements {
            row.realization
                .validate_reported_target(&row.target)
                .map_err(Diagnostic::error)?;
        }
        Ok(())
    }
}

impl ArtifactWriter {
    /// GR5: the chapter-10 trust report -- the proof-tier surface the
    /// boundary report does not carry. Written even when empty (an empty
    /// report is the honest "no semantic commitments admitted" statement).
    pub fn write_trust_report(&self, trust_report: &TrustReport) -> Result<(), Diagnostic> {
        trust_report.validate()?;
        let mut output = String::new();
        output.push_str("# Omega Trust\n\n");
        output.push_str(&format!(
            "selected provider closure report fingerprint: {:016x}\n\nselected provider closure digest: {}\n\n",
            trust_report.selected_provider_closure_report_fingerprint,
            hex_bytes(trust_report.selected_provider_closure_digest.as_bytes()),
        ));
        output.push_str(&format!(
            "admitted commitments: {}\n\n",
            trust_report.rows.len()
        ));
        for row in &trust_report.rows {
            output.push_str(&format!("- {} -- {}", row.commitment, row.provenance));
            if let Some(fingerprint) = row.machine_contract_report_fingerprint {
                output.push_str(&format!(
                    " -- machine contract report fingerprint: {fingerprint:016x}"
                ));
            }
            if let Some(commitment) = row.machine_contract_commitment {
                output.push_str(&format!(
                    " -- machine contract commitment: {}",
                    hex_bytes(&commitment.as_bytes())
                ));
            }
            if let Some(fingerprint) = row.machine_template_report_fingerprint {
                output.push_str(&format!(
                    " -- accepted template report fingerprint: {fingerprint:016x}"
                ));
            }
            if let Some(service_reach) = &row.machine_service_reach {
                output.push_str(" -- service reach: ");
                if service_reach.is_empty() {
                    output.push_str("none");
                } else {
                    output.push_str(&service_reach.join(", "));
                }
            }
            if let Some(invocations) = &row.machine_synchronous_invocations {
                output.push_str(" -- synchronous invocations: ");
                if invocations.is_empty() {
                    output.push_str("none");
                } else {
                    output.push_str(&invocations.join(", "));
                }
            }
            if let Some(may_suspend) = row.machine_may_suspend {
                output.push_str(&format!(
                    " -- may suspend: {}",
                    if may_suspend { "yes" } else { "no" }
                ));
            }
            if let Some(may_block) = row.machine_may_block {
                output.push_str(&format!(
                    " -- may block: {}",
                    if may_block { "yes" } else { "no" }
                ));
            }
            if let Some(terminates) = row.machine_terminates_guarantee {
                output.push_str(&format!(
                    " -- termination guarantee: {}",
                    if terminates { "yes" } else { "no" }
                ));
            }
            if let Some(routes) = &row.machine_crash_routes {
                output.push_str(" -- crash routes: ");
                if routes.is_empty() {
                    output.push_str("none");
                } else {
                    for (route_index, route) in routes.iter().enumerate() {
                        if route_index > 0 {
                            output.push_str(", ");
                        }
                        output.push_str(route.cause.as_str());
                        output.push('[');
                        for (guard_index, guard) in route.alternative_guards.iter().enumerate() {
                            if guard_index > 0 {
                                output.push_str(" | ");
                            }
                            output.push_str(&guard.report_text());
                        }
                        output.push(']');
                    }
                }
            }
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants it (`b.accept_boundary<..>();`)]");
            }
            output.push('\n');
        }
        output.push_str("\n## Generic accepted instances\n\n");
        output.push_str(&format!(
            "generic accepted instances: {}\n\n",
            trust_report.generic_accepted_instances.len()
        ));
        for row in &trust_report.generic_accepted_instances {
            output.push_str(&format!(
                "- accepted template: {} -- template report fingerprint: {:016x} -- instance report fingerprint: {:016x} -- instance contract report fingerprint: {:016x} -- instance contract commitment: {} -- type argument identities: {} -- const argument identities: {} -- machine argument contract report fingerprints: {} -- machine argument contract commitments: {} -- conformance argument report fingerprints: {} -- conformance argument commitments: {}\n",
                row.template_commitment,
                row.template_report_fingerprint,
                row.instance_report_fingerprint,
                row.instance_contract_report_fingerprint,
                hex_bytes(&row.instance_contract_commitment.as_bytes()),
                if row.type_argument_identities.is_empty() {
                    "none".to_owned()
                } else {
                    row.type_argument_identities.join(", ")
                },
                if row.const_argument_identities.is_empty() {
                    "none".to_owned()
                } else {
                    row.const_argument_identities.join(", ")
                },
                if row.machine_argument_contract_report_fingerprints.is_empty() {
                    "none".to_owned()
                } else {
                    row.machine_argument_contract_report_fingerprints
                        .iter()
                        .map(|fingerprint| format!("{fingerprint:016x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.machine_argument_contract_commitments.is_empty() {
                    "none".to_owned()
                } else {
                    row.machine_argument_contract_commitments
                        .iter()
                        .map(|commitment| hex_bytes(&commitment.as_bytes()))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.conformance_argument_report_fingerprints.is_empty() {
                    "none".to_owned()
                } else {
                    row.conformance_argument_report_fingerprints
                        .iter()
                        .map(|fingerprint| format!("{fingerprint:016x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.conformance_argument_commitments.is_empty() {
                    "none".to_owned()
                } else {
                    row.conformance_argument_commitments
                        .iter()
                        .map(|commitment| hex_bytes(&commitment.as_bytes()))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
        output.push_str("\n## Provider requirements\n\n");
        output.push_str(&format!(
            "provider requirements: {}\n\n",
            trust_report.provider_requirements.len()
        ));
        for row in &trust_report.provider_requirements {
            output.push_str(&format!(
                "- provider plan: {} -- plan report fingerprint: {:016x} -- plan digest: {} -- provider type: {} -- provider type package: {} -- target: {} -- provider origin package: {} -- provider package key: {} -- service schema: {} -- service schema package: {} -- calling plan report fingerprint: {} -- calling plan commitment: {} -- selected: {} -- requirement owner: {} -- requirement owner package: {} -- requirement identity: {} -- method: {} -- parameter types: {} -- result type: {} -- service reach: {} -- synchronous invocations: {} -- may suspend: {} -- may block: {} -- termination guarantee: {} -- progress premises: {} -- realization: {} -- {} -- grant selectors: {}",
                row.provider_plan,
                row.provider_plan_report_fingerprint,
                hex_bytes(row.provider_plan_digest.as_bytes()),
                if row.provider_type.is_empty() {
                    "<free external>"
                } else {
                    row.provider_type.as_str()
                },
                package_key_text(row.provider_type_package_identity),
                if row.target.is_empty() {
                    "<all>"
                } else {
                    row.target.as_str()
                },
                if row.provider_origin_package.is_empty() {
                    "<none>"
                } else {
                    row.provider_origin_package.as_str()
                },
                package_key_text(row.provider_origin_package_identity),
                row.service_schema,
                package_key_text(row.service_schema_package_identity),
                row.calling_plan_report_fingerprint
                    .map_or_else(|| "<none>".to_owned(), |value| format!("{value:016x}")),
                row.calling_plan_commitment.map_or_else(
                    || "<none>".to_owned(),
                    |commitment| hex_bytes(&commitment.as_bytes()),
                ),
                if row.selected { "yes" } else { "no" },
                row.requirement_owner,
                package_key_text(row.requirement_owner_package_identity),
                row.requirement_identity,
                row.method,
                if row.parameter_type_identities.is_empty() {
                    "<none>".to_owned()
                } else {
                    row.parameter_type_identities.join(", ")
                },
                row.result_type_identity.as_deref().unwrap_or("<none>"),
                if row.service_reach.is_empty() {
                    "none".to_owned()
                } else {
                    row.service_reach.join(", ")
                },
                if row.synchronous_invocations.is_empty() {
                    "none".to_owned()
                } else {
                    row.synchronous_invocations.join(", ")
                },
                if row.may_suspend { "yes" } else { "no" },
                if row.may_block { "yes" } else { "no" },
                if row.terminates_guarantee { "yes" } else { "no" },
                progress_premises_text(&row.termination_premises),
                row.realization.report_text(),
                row.provenance,
                if row.grant_selectors.is_empty() {
                    "none".to_owned()
                } else {
                    row.grant_selectors.join(", ")
                },
            ));
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants its provider plan]");
            }
            output.push('\n');
        }
        output.push_str("\n## Routed qualifications\n\n");
        output.push_str(&format!(
            "routed qualifications: {}\n\n",
            trust_report.qualifications.len()
        ));
        for row in &trust_report.qualifications {
            output.push_str(&format!(
                "- provider plan: {} -- plan report fingerprint: {:016x} -- plan digest: {} -- provider type: {} -- provider type package: {} -- target: {} -- provider origin package: {} -- provider package key: {} -- service schema: {} -- service schema package: {} -- calling plan report fingerprint: {} -- calling plan commitment: {} -- selected: {} -- requirement owner: {} -- requirement owner package: {} -- requirement identity: {} -- method: {} -- subject: {} -- flow: {} -- domain: {} -- carry: {} -- predicate discharge: {} -- {} -- grant selectors: {}",
                row.provider_plan,
                row.provider_plan_report_fingerprint,
                hex_bytes(row.provider_plan_digest.as_bytes()),
                if row.provider_type.is_empty() {
                    "<free external>"
                } else {
                    row.provider_type.as_str()
                },
                package_key_text(row.provider_type_package_identity),
                if row.target.is_empty() {
                    "<all>"
                } else {
                    row.target.as_str()
                },
                if row.provider_origin_package.is_empty() {
                    "<none>"
                } else {
                    row.provider_origin_package.as_str()
                },
                package_key_text(row.provider_origin_package_identity),
                row.service_schema,
                package_key_text(row.service_schema_package_identity),
                row.calling_plan_report_fingerprint
                    .map_or_else(|| "<none>".to_owned(), |value| format!("{value:016x}")),
                row.calling_plan_commitment.map_or_else(
                    || "<none>".to_owned(),
                    |commitment| hex_bytes(&commitment.as_bytes()),
                ),
                if row.selected { "yes" } else { "no" },
                row.requirement_owner,
                package_key_text(row.requirement_owner_package_identity),
                row.requirement_identity,
                row.method,
                row.subject,
                row.authority_flow,
                row.domain,
                row.effective_carry,
                if row.predicate_discharge_required {
                    "required"
                } else {
                    "none"
                },
                row.provenance,
                if row.grant_selectors.is_empty() {
                    "none".to_owned()
                } else {
                    row.grant_selectors.join(", ")
                },
            ));
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants its provider plan]");
            }
            output.push('\n');
        }
        self.write_text("trust_report.md", &output)
    }
}

fn package_key_text(identity: Option<semantic_vocabulary::PackageKeyIdentity>) -> String {
    let Some(identity) = identity else {
        return "<unbound>".to_owned();
    };
    identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn progress_premises_text(premises: &[super::TrustProgressPremiseRow]) -> String {
    if premises.is_empty() {
        return "none".to_owned();
    }
    premises
        .iter()
        .map(|premise| {
            let mut subject = match premise.subject {
                super::TrustProgressPremiseSubject::ProviderReceiver => {
                    "provider-receiver(build-bound)".to_owned()
                }
                super::TrustProgressPremiseSubject::Parameter(index) => {
                    format!("parameter:{index}")
                }
            };
            for projection in &premise.subject_projections {
                subject.push('.');
                subject.push_str(projection);
            }
            format!("{}({subject})", premise.profile)
        })
        .collect::<Vec<_>>()
        .join(", ")
}
