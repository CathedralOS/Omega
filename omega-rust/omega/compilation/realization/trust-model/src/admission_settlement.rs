//! Reconstruct the exact trust obligations a root's grants establish.
//!
//! A grant in the root's build declaration is the root's acceptance; this
//! owner resolves each grant to its exact claim or provider plan and rejects
//! grants that name nothing the program selects. It performs no policy-file
//! I/O and compares against no second acceptance record.

use crate::{NonProviderTrustGrant, TrustAdmission};
use diagnostics::Diagnostic;
use language_semantics::trust_declarations::{
    AcceptedContractDeclaration, AcceptedTemplateDeclaration, TrustDeclarationCardinality,
    TrustDeclarations,
};
use std::collections::BTreeMap;

enum PreparedTrustIdentity {
    ProviderPlan {
        report_identity: u64,
        digest: effects::provider_plan::ProviderPlanDigest,
    },
    MachineTemplate {
        report_identity: u64,
        declaration: AcceptedTemplateDeclaration,
    },
    AcceptedMachine(Result<AcceptedContractDeclaration, TrustDeclarationCardinality>),
}

struct PreparedTrustObligation {
    commitment: String,
    identity: PreparedTrustIdentity,
}

/// Reconstruct every exact trust obligation from compiler-owned semantic
/// facts. This operation performs no policy discovery and no filesystem I/O.
pub fn reconstruct_trust_obligations(
    declarations: &TrustDeclarations,
    root_grants: &[String],
    provider_plans: &[&effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
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
            declarations,
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
        let subject = crate::resolve_non_provider_trust_grant(declarations, grant)
            .map_err(|diagnostic| vec![diagnostic])?;
        let (commitment, identity) = match subject {
            NonProviderTrustGrant::AcceptedMachine(symbol) => {
                let machine =
                    crate::accepted_machine_grants::accepted_machine(declarations, symbol)
                        .map_err(|diagnostic| vec![diagnostic])?;
                let identity = match machine.template {
                    Err(cardinality) => {
                        let reason = match cardinality {
                            TrustDeclarationCardinality::Missing => {
                                "no pre-lowering template classification"
                            }
                            TrustDeclarationCardinality::Duplicate => {
                                "duplicate pre-lowering template classifications"
                            }
                        };
                        return Err(vec![Diagnostic::error(format!(
                            "accepted machine `{}` has {reason}",
                            machine.name
                        ))]);
                    }
                    Ok(Some(declaration)) => PreparedTrustIdentity::MachineTemplate {
                        report_identity: declaration.report_fingerprint,
                        declaration,
                    },
                    Ok(None) => PreparedTrustIdentity::AcceptedMachine(machine.contract),
                };
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
                declaration,
            } => TrustAdmission::for_machine_template(
                row.commitment.clone(),
                report_identity,
                declaration,
            ),
            PreparedTrustIdentity::AcceptedMachine(contract) => {
                let contract = contract.map_err(|cardinality| {
                    let reason = match cardinality {
                        TrustDeclarationCardinality::Missing => {
                            "no exact checked machine contract plan"
                        }
                        TrustDeclarationCardinality::Duplicate => {
                            "duplicate exact checked machine contract plans"
                        }
                    };
                    vec![Diagnostic::error(format!(
                        "accepted trust obligation `{}` has {reason}",
                        row.commitment
                    ))]
                })?;
                TrustAdmission::for_machine_contract(
                    row.commitment.clone(),
                    contract.report_fingerprint,
                    contract,
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
