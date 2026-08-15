//! GR5/GR6 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment, plus exact provider-requirement and routed
//! qualification rows copied from normalized provider plans. Domain
//! introductions, accepted facts, provider plans, and their requirement blast
//! radius retain root-grant or dev-active provenance; the latter carries a
//! standing warning.

use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, TrustProviderRealization, TrustProviderRequirementRow, TrustQualificationRow,
    TrustReport, TrustReportRow,
};
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
    let mut recognized_provider_grants = Vec::new();
    // PRV3: derived provider plans -- one row each, dev-active with the
    // standing warning until the final build grants the plan by name (or
    // its trait leaf), fingerprint shown so drift is visible at a glance.
    for plan in provider_plans {
        let leaf = plan.schema.trait_name.as_str();
        let selected = selected_provider_plans
            .iter()
            .any(|selected| selected == &plan.name);
        let grant_selectors = selected
            .then(|| {
                root_grants
                    .iter()
                    .filter(|grant| *grant == &plan.name || *grant == leaf)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for selector in &grant_selectors {
            if !recognized_provider_grants.contains(selector) {
                recognized_provider_grants.push(selector.clone());
            }
        }
        let granted = !grant_selectors.is_empty();
        let provenance = if granted {
            "root grant (build.omg)"
        } else {
            "own-package (dev-active)"
        };
        let covered = plan
            .schema
            .methods
            .iter()
            .filter(|method| {
                plan.rows
                    .iter()
                    .any(|row| plan.schema.row_binds_method(row, method))
            })
            .count();
        report.rows.push(TrustReportRow {
            commitment: format!(
                "provider plan: {} [{:016x}] coverage {covered}/{} selected: {}",
                plan.name,
                plan.identity_fingerprint(),
                plan.schema.methods.len(),
                if selected { "yes" } else { "no" },
            ),
            provenance: provenance.to_owned(),
            standing_warning: !granted,
        });
        let mut bound_methods = Vec::with_capacity(plan.rows.len());
        for row in &plan.rows {
            let (method_index, method) = plan
                .schema
                .methods
                .iter()
                .enumerate()
                .find(|(_, method)| plan.schema.row_binds_method(row, method))
                .expect("validated provider rows bind one exact schema requirement");
            bound_methods.push((method_index, method));
            report
                .provider_requirements
                .push(TrustProviderRequirementRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_identity: row.requirement_identity.clone(),
                    method: row.method.clone(),
                    service_reach: method.service_reach.clone(),
                    synchronous_invocations: method.synchronous_invocations.clone(),
                    may_suspend: method.may_suspend,
                    may_block: method.may_block,
                    realization: trust_provider_realization(&row.binding),
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
                    standing_warning: !granted,
                });
        }
        // Preserve schema declaration order while excluding every unbound
        // requirement from a partial candidate's qualification blast radius.
        bound_methods.sort_unstable_by_key(|(method_index, _)| *method_index);
        for (_, method) in bound_methods {
            for claim in &method.entry_claims {
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_identity: method.requirement_identity.clone(),
                    method: method.name.clone(),
                    subject: format!("parameter:{}", claim.parameter_index),
                    authority_flow: claim.authority_flow.as_str().to_owned(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry.to_string(),
                    predicate_discharge_required: claim.predicate_body.is_present(),
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
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
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_identity: method.requirement_identity.clone(),
                    method: method.name.clone(),
                    subject: "result".to_owned(),
                    authority_flow: "returns".to_owned(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry.to_string(),
                    predicate_discharge_required: false,
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
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
    // Grants naming anything other than a declared domain, an accepted
    // machine, or an already-reported selected provider plan surface as bare
    // accepted-fact rows (the report shows every grant, private or public).
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
        let names_selected_provider = recognized_provider_grants.contains(grant);
        if !names_domain && !names_accepted && !names_selected_provider {
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

fn trust_provider_realization(
    binding: &omega_effects::provider_plan::ProviderBinding,
) -> TrustProviderRealization {
    use omega_effects::provider_plan::ProviderBinding;

    match binding {
        ProviderBinding::Import { library, symbol } => TrustProviderRealization::Import {
            library: library.clone(),
            symbol: symbol.clone(),
        },
        ProviderBinding::Syscall { number } => {
            TrustProviderRealization::Syscall { number: *number }
        }
        ProviderBinding::CompilerIntrinsic { name } => {
            TrustProviderRealization::CompilerIntrinsic { name: name.clone() }
        }
        ProviderBinding::VtableSlot { index } => {
            TrustProviderRealization::VtableSlot { index: *index }
        }
        ProviderBinding::VtableField { table, field } => TrustProviderRealization::VtableField {
            table: table.clone(),
            field: field.clone(),
        },
        ProviderBinding::TableFunction { table, field } => {
            TrustProviderRealization::TableFunction {
                table: table.clone(),
                field: field.clone(),
            }
        }
        ProviderBinding::CheckedAdapter { machine } => TrustProviderRealization::CheckedAdapter {
            machine: machine.clone(),
        },
    }
}
