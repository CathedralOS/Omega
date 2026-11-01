//! Publish the complete accepted-machine census from one immutable judgment.
//!
//! Eligibility is source semantics: specialized accepted instances are not new
//! grant subjects. Realization receives every eligible declaration, including
//! unused ones, with exact template/contract facts and deferred cardinality
//! failures. It never reopens the source to decide what a grant named.

use crate::CheckedTrees;
use language_semantics::trust_declarations::{
    AcceptedContractDeclaration, AcceptedMachineDeclaration, AcceptedTemplateDeclaration,
    TrustDeclarationCardinality, TrustDeclarations,
};

pub(crate) fn collect(checked: &CheckedTrees) -> TrustDeclarations {
    let machines = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                && !checked
                    .typed
                    .machine_specializations
                    .iter()
                    .any(|specialization| {
                        specialization.accepted_template_commitment.is_some()
                            && specialization.instance == machine.symbol
                            && specialization.instance != specialization.template
                    })
        })
        .map(|machine| {
            let mut templates = checked
                .facts
                .accepted_template_identities
                .rows()
                .iter()
                .filter(|row| row.machine == machine.symbol);
            let template = match (templates.next(), templates.next()) {
                (None, _) => Err(TrustDeclarationCardinality::Missing),
                (Some(_), Some(_)) => Err(TrustDeclarationCardinality::Duplicate),
                (Some(row), None) => Ok(row.identity.map(|identity| AcceptedTemplateDeclaration {
                    report_fingerprint: identity.report_fingerprint,
                    commitment: identity.commitment.as_bytes(),
                })),
            };
            let mut contracts = checked
                .facts
                .contract_plans
                .machines
                .iter()
                .filter(|plan| plan.machine == machine.symbol);
            let contract = match (contracts.next(), contracts.next()) {
                (None, _) => Err(TrustDeclarationCardinality::Missing),
                (Some(_), Some(_)) => Err(TrustDeclarationCardinality::Duplicate),
                (Some(plan), None) => Ok(AcceptedContractDeclaration {
                    report_fingerprint: plan.report_fingerprint,
                    commitment: plan.commitment.as_bytes(),
                }),
            };
            AcceptedMachineDeclaration {
                machine: machine.symbol,
                name: machine.name.as_str().to_owned(),
                template,
                contract,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    TrustDeclarations { machines }
}

#[cfg(test)]
mod tests;
