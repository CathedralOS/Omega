//! GR5 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment. Today's rows are the SEALED-DOMAIN
//! INTRODUCTIONS: every domain declared in the compilation unit is
//! own-package and dev-active (grant locality v1, mirroring the
//! MintAuthority consult in omega-validation's recasts), so each carries
//! the standing warning until GR3's root grants land and flip its
//! provenance. Progress profiles, accepted facts, and provider plans join
//! as their consumers wire in (GR6).

use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{ArtifactWriter, TrustReport, TrustReportRow};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

pub(super) fn write_trust_report(
    options: &CompileOptions,
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
) -> Result<(), Vec<Diagnostic>> {
    let mut report = TrustReport::default();
    // PRV3: derived provider plans -- one row each, dev-active with the
    // standing warning until the final build grants the plan by name (or
    // its trait leaf), fingerprint shown so drift is visible at a glance.
    for plan in provider_plans {
        let leaf = plan.schema.trait_name.as_str();
        let granted = root_grants
            .iter()
            .any(|grant| grant == &plan.name || grant == leaf);
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
            provenance: if granted {
                "root grant (build.omg)".to_owned()
            } else {
                "own-package (dev-active)".to_owned()
            },
            standing_warning: !granted,
        });
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
        if machine.supply_mode != omega_core::semantics::MachineSupplyMode::Accepted {
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
                || Some(grant.as_str())
                    == domain.name.as_str().rsplit("::").next()
        });
        let names_accepted = typed.machines().iter().any(|machine| {
            machine.supply_mode == omega_core::semantics::MachineSupplyMode::Accepted
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
