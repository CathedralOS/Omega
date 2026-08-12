//! GR5/GR6 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment, plus exact routed qualification rows copied
//! from normalized provider schemas. Domain introductions, accepted facts,
//! provider plans, and their qualification blast radius retain root-grant or
//! dev-active provenance; the latter carries a standing warning.

use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{ArtifactWriter, TrustQualificationRow, TrustReport, TrustReportRow};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

pub(super) fn write_trust_report(
    options: &CompileOptions,
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &[String],
) -> Result<(), Vec<Diagnostic>> {
    let mut report = TrustReport::default();
    // PRV3: derived provider plans -- one row each, dev-active with the
    // standing warning until the final build grants the plan by name (or
    // its trait leaf), fingerprint shown so drift is visible at a glance.
    for plan in provider_plans {
        let leaf = plan.schema.trait_name.as_str();
        let selected = selected_provider_plans
            .iter()
            .any(|selected| selected == &plan.name);
        let granted = selected
            && root_grants
                .iter()
                .any(|grant| grant == &plan.name || grant == leaf);
        let provenance = if granted {
            "root grant (build.omg)"
        } else {
            "own-package (dev-active)"
        };
        let covered = plan
            .schema
            .methods
            .iter()
            .filter(|method| plan.rows.iter().any(|row| row.method == method.name))
            .count();
        report.rows.push(TrustReportRow {
            commitment: format!(
                "provider plan: {} [{:016x}] coverage {covered}/{}",
                plan.name,
                plan.identity_fingerprint(),
                plan.schema.methods.len()
            ),
            provenance: provenance.to_owned(),
            standing_warning: !granted,
        });
        for method in &plan.schema.methods {
            for claim in &method.entry_claims {
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    requirement: method.requirement_identity.clone(),
                    method: method.name.clone(),
                    subject: format!("parameter:{}", claim.parameter_index),
                    authority_flow: claim.authority_flow.as_str().to_owned(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry.to_string(),
                    predicate_discharge_required: claim.predicate_body.is_present(),
                    provenance: provenance.to_owned(),
                    standing_warning: !granted,
                });
            }
            for claim in &method.result_claims {
                // ServiceResultClaim contains only bodyless routed results;
                // predicate-bearing establishment is deliberately absent from
                // this generic provider-result carrier.
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    requirement: method.requirement_identity.clone(),
                    method: method.name.clone(),
                    subject: "result".to_owned(),
                    authority_flow: "returns".to_owned(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry.to_string(),
                    predicate_discharge_required: false,
                    provenance: provenance.to_owned(),
                    standing_warning: !granted,
                });
            }
        }
    }
    for domain in typed.domain_definitions() {
        if !domain.semantic_id.is_valid() {
            continue;
        }
        // A root grant naming this domain (by full rendered name or leaf)
        // flips its provenance and retires the standing warning (GR3).
        let leaf = domain
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(domain.name.as_str());
        let granted = root_grants
            .iter()
            .any(|grant| grant == domain.name.as_str() || grant == leaf);
        report.rows.push(TrustReportRow {
            commitment: format!("domain introduction: {}", domain.name.as_str()),
            provenance: if granted {
                "root grant (build.omg)".to_owned()
            } else {
                "own-package (dev-active)".to_owned()
            },
            standing_warning: !granted,
        });
    }
    // ACCEPTED machines (bodyless boundary axioms, GR6d): one row each --
    // own-package dev-active with the standing warning, or root-granted
    // when build.omg names the machine.
    for machine in typed.machines() {
        if machine.supply_mode != psi_language_semantics::MachineSupplyMode::Accepted {
            continue;
        }
        let leaf = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(machine.name.as_str());
        let granted = root_grants
            .iter()
            .any(|grant| grant == machine.name.as_str() || grant == leaf);
        report.rows.push(TrustReportRow {
            commitment: format!("accepted fact: {}", machine.name.as_str()),
            provenance: if granted {
                "root grant (build.omg)".to_owned()
            } else {
                "own-package (dev-active)".to_owned()
            },
            standing_warning: !granted,
        });
    }
    // Grants naming anything other than a declared domain OR an accepted
    // machine surface as bare accepted-fact rows (the report shows every
    // grant, private or public).
    for grant in root_grants {
        let names_domain = typed.domain_definitions().iter().any(|domain| {
            grant == domain.name.as_str()
                || Some(grant.as_str()) == domain.name.as_str().rsplit("::").next()
        });
        let names_accepted = typed.machines().iter().any(|machine| {
            machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
                && (grant == machine.name.as_str()
                    || Some(grant.as_str()) == machine.name.as_str().rsplit("::").next())
        });
        if !names_domain && !names_accepted {
            report.rows.push(TrustReportRow {
                commitment: format!("accepted fact: {grant}"),
                provenance: "root grant (build.omg)".to_owned(),
                standing_warning: false,
            });
        }
    }

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_trust_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}
