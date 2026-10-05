//! Reconstruct the exact trust obligations a root's grants establish.
//!
//! A grant in the root's build declaration is the root's acceptance; this
//! owner resolves each grant to its exact claim or provider plan and rejects
//! grants that name nothing the program selects. It performs no policy-file
//! I/O and compares against no second acceptance record.

use crate::{AcceptedTemplateClassifications, NonProviderTrustGrant, TrustAdmission};
use diagnostics::Diagnostic;
use std::collections::BTreeMap;
use typed_trees::TypedTrees;

enum PreparedTrustIdentity {
    ProviderPlan {
        report_identity: u64,
        digest: effects::provider_plan::ProviderPlanDigest,
    },
    MachineTemplate {
        report_identity: u64,
        commitment: typed_trees::typed_trees::MachineTemplateCommitment,
    },
    AcceptedMachine(symbols::SymbolHandle),
}

struct PreparedTrustObligation {
    commitment: String,
    identity: PreparedTrustIdentity,
}

/// Reconstruct every exact trust obligation from compiler-owned semantic
/// facts. This operation performs no policy discovery and no filesystem I/O.
pub fn reconstruct_trust_obligations(
    typed: &TypedTrees,
    checked: &checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    accepted_template_classifications: &AcceptedTemplateClassifications,
    package_aware: bool,
) -> Result<Vec<TrustAdmission>, Vec<Diagnostic>> {
    reconstruct_obligations(
        typed,
        checked,
        root_grants,
        provider_plans,
        selected_provider_plans,
        accepted_template_classifications,
        package_aware,
    )
}

fn reconstruct_obligations(
    typed: &TypedTrees,
    checked: &checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    accepted_template_classifications: &AcceptedTemplateClassifications,
    package_aware: bool,
) -> Result<Vec<TrustAdmission>, Vec<Diagnostic>> {
    let mut rows = Vec::<PreparedTrustObligation>::new();
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    if package_aware {
        crate::root_grants::reject_package_grants_with_providers(
            typed,
            root_grants,
            &provider_grants,
        )?;
    }
    for grant in root_grants {
        if let Some(provider_grant) = provider_grants
            .iter()
            .find(|provider_grant| provider_grant.selector == *grant)
        {
            let commitment = provider_grant.commitment();
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustObligation {
                    commitment,
                    identity: PreparedTrustIdentity::ProviderPlan {
                        report_identity: provider_grant.selected_plan_report_identity,
                        digest: provider_grant.selected_plan_digest,
                    },
                });
            }
            continue;
        }
        let subject = crate::resolve_non_provider_trust_grant(typed, grant)
            .map_err(|diagnostic| vec![diagnostic])?;
        let (commitment, identity) = match subject {
            NonProviderTrustGrant::AcceptedMachine(symbol) => {
                let machine = crate::accepted_machine_grants::accepted_machine(typed, symbol)
                    .map_err(|diagnostic| vec![diagnostic])?;
                let identity = accepted_template_classifications
                    .for_machine(machine.symbol, machine.name.as_str())
                    .map_err(|diagnostic| vec![diagnostic])?
                    .map(|identity| PreparedTrustIdentity::MachineTemplate {
                        report_identity: identity.report_fingerprint(),
                        commitment: identity.commitment(),
                    })
                    .unwrap_or(PreparedTrustIdentity::AcceptedMachine(machine.symbol));
                (
                    format!("accepted fact: {}", machine.name.as_str()),
                    identity,
                )
            }
        };
        if !rows.iter().any(|row| row.commitment == commitment) {
            rows.push(PreparedTrustObligation {
                commitment,
                identity,
            });
        }
    }

    let mut resolved = BTreeMap::new();
    for row in rows {
        let admission = match row.identity {
            PreparedTrustIdentity::ProviderPlan {
                report_identity,
                digest,
            } => TrustAdmission::for_provider_plan(row.commitment.clone(), report_identity, digest),
            PreparedTrustIdentity::MachineTemplate {
                report_identity,
                commitment,
            } => TrustAdmission::for_machine_template(
                row.commitment.clone(),
                report_identity,
                commitment,
            ),
            PreparedTrustIdentity::AcceptedMachine(machine) => {
                let mut matches = checked
                    .facts
                    .contract_plans
                    .machines
                    .iter()
                    .filter(|plan| plan.machine == machine);
                let plan = matches.next().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "accepted trust obligation `{}` has no exact checked machine contract plan",
                        row.commitment
                    ))]
                })?;
                if matches.next().is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "accepted trust obligation `{}` has duplicate exact checked machine contract plans",
                        row.commitment
                    ))]);
                }
                TrustAdmission::for_machine_contract(
                    row.commitment.clone(),
                    plan.report_fingerprint,
                    plan.commitment,
                )
            }
        }
        .map_err(|error| vec![Diagnostic::error(error)])?;
        if resolved.insert(row.commitment.clone(), admission).is_some() {
            return Err(vec![Diagnostic::error(format!(
                "current trust obligation set contains duplicate commitment `{}`",
                row.commitment
            ))]);
        }
    }
    Ok(resolved.into_values().collect())
}
