use crate::{AcceptedTemplateClassifications, NonProviderTrustGrant};
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use typed_trees::TypedTrees;

/// Domain-separated collision-resistant identity of one persisted owner
/// admission. The human-readable commitment remains part of admission
/// authority, but compact report coordinates do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrustAdmissionDigest([u8; 32]);

impl TrustAdmissionDigest {
    pub fn from_digest(digest: [u8; 32]) -> Result<Self, &'static str> {
        if digest == [0; 32] {
            return Err("trust-admission digests must not be all zero");
        }
        Ok(Self(digest))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for TrustAdmissionDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum TrustAdmissionSubject {
    ProviderPlan,
    MachineTemplate,
    MachineContract,
}

impl TrustAdmissionSubject {
    const fn domain(self) -> &'static [u8] {
        match self {
            Self::ProviderPlan => b"provider-plan",
            Self::MachineTemplate => b"machine-template",
            Self::MachineContract => b"machine-contract",
        }
    }
}

fn trust_admission_digest(
    subject: TrustAdmissionSubject,
    commitment: &str,
    underlying: &[u8; 32],
) -> TrustAdmissionDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.trust-admission.v1\0");
    digest.update((subject.domain().len() as u64).to_le_bytes());
    digest.update(subject.domain());
    digest.update((commitment.len() as u64).to_le_bytes());
    digest.update(commitment.as_bytes());
    digest.update(underlying);
    TrustAdmissionDigest(digest.finalize().into())
}

/// One exact owner-policy admission consumed by compilation.
#[derive(Debug, Clone)]
pub struct TrustAdmission {
    commitment: String,
    digest: TrustAdmissionDigest,
    report_identity: Option<u64>,
}

impl TrustAdmission {
    fn validate_commitment(commitment: &str) -> Result<(), &'static str> {
        if commitment.is_empty()
            || commitment.contains('\n')
            || commitment.contains('\r')
            || commitment.contains('\0')
        {
            return Err("trust-admission commitments must be nonempty single-line text");
        }
        Ok(())
    }

    fn derived(
        commitment: String,
        subject: TrustAdmissionSubject,
        underlying: &[u8; 32],
        report_identity: Option<u64>,
    ) -> Result<Self, &'static str> {
        Self::validate_commitment(&commitment)?;
        Ok(Self {
            digest: trust_admission_digest(subject, &commitment, underlying),
            commitment,
            report_identity,
        })
    }

    pub fn for_provider_plan(
        commitment: String,
        report_identity: u64,
        provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
    ) -> Result<Self, &'static str> {
        Self::derived(
            commitment,
            TrustAdmissionSubject::ProviderPlan,
            provider_plan_digest.as_bytes(),
            Some(report_identity),
        )
    }

    pub fn for_machine_template(
        commitment: String,
        report_identity: u64,
        template_commitment: typed_trees::typed_trees::MachineTemplateCommitment,
    ) -> Result<Self, &'static str> {
        Self::derived(
            commitment,
            TrustAdmissionSubject::MachineTemplate,
            &template_commitment.as_bytes(),
            Some(report_identity),
        )
    }

    pub fn for_machine_contract(
        commitment: String,
        report_identity: u64,
        contract_commitment: checked_trees::MachineContractCommitment,
    ) -> Result<Self, &'static str> {
        Self::derived(
            commitment,
            TrustAdmissionSubject::MachineContract,
            &contract_commitment.as_bytes(),
            Some(report_identity),
        )
    }

    /// Reconstruct one owner-persisted admission. The digest is already the
    /// authority value; compact display data is intentionally absent.
    pub fn from_persisted(
        commitment: String,
        digest: TrustAdmissionDigest,
    ) -> Result<Self, &'static str> {
        Self::validate_commitment(&commitment)?;
        Ok(Self {
            commitment,
            digest,
            report_identity: None,
        })
    }

    pub fn commitment(&self) -> &str {
        &self.commitment
    }

    pub const fn digest(&self) -> TrustAdmissionDigest {
        self.digest
    }

    pub const fn report_identity(&self) -> Option<u64> {
        self.report_identity
    }
}

impl PartialEq for TrustAdmission {
    fn eq(&self, other: &Self) -> bool {
        (self.commitment.as_str(), self.digest) == (other.commitment.as_str(), other.digest)
    }
}

impl Eq for TrustAdmission {}

impl PartialOrd for TrustAdmission {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TrustAdmission {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.commitment.as_str(), self.digest).cmp(&(other.commitment.as_str(), other.digest))
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
                    identity: PreparedTrustIdentity::ProviderPlan {
                        report_identity: provider_grant.selected_plan_report_identity,
                        digest: provider_grant.selected_plan_digest,
                    },
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
                "compile request supplied duplicate trust commitment `{}` [{}]",
                admission.commitment, admission.digest,
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

    fn admission(name: &str, report_identity: u64, digest_byte: u8) -> TrustAdmission {
        TrustAdmission::for_machine_contract(
            name.to_owned(),
            report_identity,
            checked_trees::MachineContractCommitment::from_digest([digest_byte; 32]),
        )
        .unwrap()
    }

    #[test]
    fn settlement_names_consumed_unresolved_and_unused_rows() {
        let required = vec![admission("a", 1, 1), admission("b", 2, 2)];
        let accepted = vec![admission("a", 1, 1), admission("b", 2, 3)];
        let settlement = settle_trust_admissions(required.clone(), &accepted).unwrap();
        assert_eq!(settlement.required(), required);
        assert_eq!(settlement.consumed(), &[admission("a", 1, 1)]);
        assert_eq!(settlement.unresolved(), &[admission("b", 2, 2)]);
        assert_eq!(settlement.unused(), &[admission("b", 2, 3)]);
        assert!(!settlement.is_exactly_admitted());
    }

    #[test]
    fn duplicate_request_admissions_are_rejected() {
        let row = admission("a", 1, 1);
        assert!(settle_trust_admissions(vec![row.clone()], &[row.clone(), row]).is_err());
    }

    #[test]
    fn compact_report_identity_is_excluded_from_admission_authority() {
        let first = admission("accepted fact: A", 1, 7);
        let second = admission("accepted fact: A", 2, 7);

        assert_eq!(first, second);
        assert_eq!(first.cmp(&second), Ordering::Equal);
        assert_ne!(first.report_identity(), second.report_identity());
    }

    #[test]
    fn same_human_commitment_with_different_strong_identity_does_not_settle() {
        let required = admission("accepted fact: A", 1, 7);
        let accepted = admission("accepted fact: A", 1, 8);

        let settlement =
            settle_trust_admissions(vec![required.clone()], std::slice::from_ref(&accepted))
                .expect("distinct strong admissions are not duplicate request rows");
        assert_eq!(settlement.unresolved(), &[required]);
        assert_eq!(settlement.unused(), &[accepted]);
    }

    #[test]
    fn subject_kind_domain_separates_equal_underlying_commitments() {
        let commitment = "accepted fact: A".to_owned();
        let template = TrustAdmission::for_machine_template(
            commitment.clone(),
            1,
            typed_trees::typed_trees::MachineTemplateCommitment::from_digest([7; 32]),
        )
        .unwrap();
        let contract = TrustAdmission::for_machine_contract(
            commitment,
            1,
            checked_trees::MachineContractCommitment::from_digest([7; 32]),
        )
        .unwrap();

        assert_ne!(template.digest(), contract.digest());
        assert_ne!(template, contract);
    }

    #[test]
    fn persisted_zero_digest_is_rejected() {
        assert_eq!(
            TrustAdmissionDigest::from_digest([0; 32]),
            Err("trust-admission digests must not be all zero"),
        );
    }
}
