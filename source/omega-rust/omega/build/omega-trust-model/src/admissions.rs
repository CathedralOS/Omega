use crate::{AcceptedTemplateClassifications, NonProviderTrustGrant};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use std::collections::{BTreeMap, BTreeSet};

/// One exact owner-policy admission consumed by compilation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrustAdmission {
    commitment: String,
    identity: u64,
}

impl TrustAdmission {
    pub fn new(commitment: String, identity: u64) -> Result<Self, &'static str> {
        if commitment.is_empty()
            || commitment.contains('\n')
            || commitment.contains('\r')
            || commitment.contains('\0')
        {
            return Err("trust-admission commitments must be nonempty single-line text");
        }
        Ok(Self {
            commitment,
            identity,
        })
    }

    pub fn commitment(&self) -> &str {
        &self.commitment
    }

    pub const fn identity(&self) -> u64 {
        self.identity
    }
}

/// Exact comparison between compiler-reconstructed obligations and the
/// admissions explicitly supplied in the compile request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustAdmissionSettlement {
    required: Vec<TrustAdmission>,
    consumed: Vec<TrustAdmission>,
    unresolved: Vec<TrustAdmission>,
    unused: Vec<TrustAdmission>,
}

impl TrustAdmissionSettlement {
    pub fn required(&self) -> &[TrustAdmission] {
        &self.required
    }

    pub fn consumed(&self) -> &[TrustAdmission] {
        &self.consumed
    }

    pub fn unresolved(&self) -> &[TrustAdmission] {
        &self.unresolved
    }

    pub fn unused(&self) -> &[TrustAdmission] {
        &self.unused
    }

    pub fn is_exactly_admitted(&self) -> bool {
        self.unresolved.is_empty() && self.unused.is_empty()
    }
}

enum PreparedTrustIdentity {
    Ready(u64),
    AcceptedMachine(psi_symbols::SymbolHandle),
}

struct PreparedTrustObligation {
    commitment: String,
    identity: PreparedTrustIdentity,
}

/// Reconstruct every exact trust obligation from compiler-owned semantic
/// facts. This operation performs no policy discovery and no filesystem I/O.
pub fn reconstruct_trust_obligations(
    typed: &TypedTrees,
    checked: &psi_checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    accepted_template_classifications: &AcceptedTemplateClassifications,
    package_aware: bool,
) -> Result<Vec<TrustAdmission>, Vec<Diagnostic>> {
    if package_aware {
        crate::reject_package_non_provider_grants(
            typed,
            root_grants,
            provider_plans,
            selected_provider_plans,
        )?;
    }
    let mut rows = Vec::<PreparedTrustObligation>::new();
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    for grant in root_grants {
        if let Some(provider_grant) = provider_grants
            .iter()
            .find(|provider_grant| provider_grant.selector == *grant)
        {
            let commitment = provider_grant.commitment();
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustObligation {
                    commitment,
                    identity: PreparedTrustIdentity::Ready(provider_grant.selected_plan_identity),
                });
            }
            continue;
        }
        let (commitment, identity) = match crate::resolve_non_provider_trust_grant(typed, grant)
            .map_err(|diagnostic| vec![diagnostic])?
        {
            NonProviderTrustGrant::AcceptedMachine(symbol) => {
                let machine = crate::grants::accepted_machine(typed, symbol)
                    .map_err(|diagnostic| vec![diagnostic])?;
                let identity = accepted_template_classifications
                    .for_machine(machine.symbol, machine.name.as_str())
                    .map_err(|diagnostic| vec![diagnostic])?
                    .map(PreparedTrustIdentity::Ready)
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
        let identity = match row.identity {
            PreparedTrustIdentity::Ready(identity) => identity,
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
                plan.fingerprint
            }
        };
        if resolved.insert(row.commitment.clone(), identity).is_some() {
            return Err(vec![Diagnostic::error(format!(
                "current trust obligation set contains duplicate commitment `{}`",
                row.commitment
            ))]);
        }
    }
    resolved
        .into_iter()
        .map(|(commitment, identity)| {
            TrustAdmission::new(commitment, identity)
                .map_err(|error| vec![Diagnostic::error(error)])
        })
        .collect()
}

pub fn settle_trust_admissions(
    required: Vec<TrustAdmission>,
    accepted: &[TrustAdmission],
) -> Result<TrustAdmissionSettlement, Diagnostic> {
    let mut accepted_set = BTreeSet::new();
    let mut accepted_commitments = BTreeSet::new();
    for admission in accepted {
        if !accepted_commitments.insert(admission.commitment.clone())
            || !accepted_set.insert(admission.clone())
        {
            return Err(Diagnostic::error(format!(
                "compile request supplied duplicate trust commitment `{}` [{:016x}]",
                admission.commitment, admission.identity,
            )));
        }
    }
    let required_set = required.iter().cloned().collect::<BTreeSet<_>>();
    let consumed = required_set.intersection(&accepted_set).cloned().collect();
    let unresolved = required_set.difference(&accepted_set).cloned().collect();
    let unused = accepted_set.difference(&required_set).cloned().collect();
    Ok(TrustAdmissionSettlement {
        required,
        consumed,
        unresolved,
        unused,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admission(name: &str, identity: u64) -> TrustAdmission {
        TrustAdmission::new(name.to_owned(), identity).unwrap()
    }

    #[test]
    fn settlement_names_consumed_unresolved_and_unused_rows() {
        let required = vec![admission("a", 1), admission("b", 2)];
        let accepted = vec![admission("a", 1), admission("b", 3)];
        let settlement = settle_trust_admissions(required.clone(), &accepted).unwrap();
        assert_eq!(settlement.required(), required);
        assert_eq!(settlement.consumed(), &[admission("a", 1)]);
        assert_eq!(settlement.unresolved(), &[admission("b", 2)]);
        assert_eq!(settlement.unused(), &[admission("b", 3)]);
        assert!(!settlement.is_exactly_admitted());
    }

    #[test]
    fn duplicate_request_admissions_are_rejected() {
        let row = admission("a", 1);
        assert!(settle_trust_admissions(vec![row.clone()], &[row.clone(), row]).is_err());
    }
}
