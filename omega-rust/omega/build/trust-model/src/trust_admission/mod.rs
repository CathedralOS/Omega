//! Exact admission values: semantic subjects, strong digests, and identity comparison.

use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fmt;

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
