//! Bind the caller-stack contribution of an opaque provider to the selected plan,
//! requirement and admission receipt. Native call records carry this evidence
//! through register allocation, frame accounting and image publication.
#![forbid(unsafe_code)]

use wire::fnv1a::Fnv1a;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackContributionError(pub String);

impl std::fmt::Display for StackContributionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StackContributionError {}

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, StackContributionError> {
                if identity == 0 {
                    return Err(StackContributionError(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

macro_rules! commitment_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }

            pub fn is_zero(self) -> bool {
                self.0 == [0; 32]
            }
        }
    };
}

normalized_id!(
    AdmittedStackContributionReportId,
    "admitted-stack-contribution-report"
);
normalized_id!(
    SameStackContributionAdmissionReceiptId,
    "same-stack-contribution-admission-receipt"
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameStackContributionAdmissionCandidate {
    pub provider_plan_report_identity: u64, // Historical compact report coordinate; not admission authority.
    pub provider_plan_commitment: SameStackProviderPlanCommitment, // Strong SHA-256 identity of the selected provider plan.
    pub requirement_identity: terminal_psi::TerminalIdentityIndex,
    pub receipt: SameStackContributionAdmissionReceiptId,
    pub bytes: u64,
    pub alignment: u64,
}

commitment_digest!(SameStackProviderPlanCommitment);
commitment_digest!(SameStackContributionCommitment);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdmittedSameStackContribution {
    report_identity: AdmittedStackContributionReportId,
    commitment: SameStackContributionCommitment,
    provider_plan_report_identity: u64,
    provider_plan_commitment: SameStackProviderPlanCommitment,
    requirement_identity: terminal_psi::TerminalIdentityIndex,
    receipt: SameStackContributionAdmissionReceiptId,
    bytes: u64,
    alignment: u64,
}

impl AdmittedSameStackContribution {
    pub const fn report_identity(&self) -> AdmittedStackContributionReportId {
        self.report_identity
    }

    pub const fn commitment(&self) -> SameStackContributionCommitment {
        self.commitment
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn provider_plan_commitment(&self) -> SameStackProviderPlanCommitment {
        self.provider_plan_commitment
    }

    pub const fn requirement_identity(&self) -> terminal_psi::TerminalIdentityIndex {
        self.requirement_identity
    }

    pub const fn receipt(&self) -> SameStackContributionAdmissionReceiptId {
        self.receipt
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}

pub fn admit_same_stack_contribution(
    candidate: SameStackContributionAdmissionCandidate,
    selected_provider_plan_report_identity: u64,
    selected_provider_plan_commitment: SameStackProviderPlanCommitment,
    selected_requirement_identity: terminal_psi::TerminalIdentityIndex,
) -> Result<AdmittedSameStackContribution, StackContributionError> {
    if selected_provider_plan_report_identity == 0 {
        return Err(StackContributionError(
            "selected provider-plan report identity for same-stack admission cannot be zero".into(),
        ));
    }
    if selected_provider_plan_commitment.is_zero() {
        return Err(StackContributionError(
            "selected provider-plan commitment for same-stack admission cannot be zero".into(),
        ));
    }
    if candidate.provider_plan_report_identity != selected_provider_plan_report_identity {
        return Err(StackContributionError(format!(
            "same-stack admission provider-plan report identity 0x{:016x} does not match selected report identity 0x{selected_provider_plan_report_identity:016x}",
            candidate.provider_plan_report_identity
        )));
    }
    if candidate.provider_plan_commitment.is_zero()
        || candidate.provider_plan_commitment != selected_provider_plan_commitment
    {
        return Err(StackContributionError(
            "same-stack admission provider-plan commitment does not match the exact selected plan"
                .into(),
        ));
    }
    if selected_requirement_identity.is_absent() {
        return Err(StackContributionError(
            "selected requirement identity for same-stack admission cannot be absent".into(),
        ));
    }
    if candidate.requirement_identity != selected_requirement_identity {
        return Err(StackContributionError(
            "same-stack admission requirement identity does not match the selected identity".into(),
        ));
    }
    if !candidate.alignment.is_power_of_two() {
        return Err(StackContributionError(format!(
            "same-stack admission alignment {} is not a nonzero power of two",
            candidate.alignment
        )));
    }

    let report_identity =
        AdmittedStackContributionReportId(admitted_contribution_report_fingerprint(&candidate));
    let commitment = admitted_contribution_commitment(&candidate);
    Ok(AdmittedSameStackContribution {
        report_identity,
        commitment,
        provider_plan_report_identity: candidate.provider_plan_report_identity,
        provider_plan_commitment: candidate.provider_plan_commitment,
        requirement_identity: candidate.requirement_identity,
        receipt: candidate.receipt,
        bytes: candidate.bytes,
        alignment: candidate.alignment,
    })
}

fn admitted_contribution_report_fingerprint(
    candidate: &SameStackContributionAdmissionCandidate,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(candidate.provider_plan_report_identity);
    for byte in candidate.provider_plan_commitment.as_bytes() {
        hash.byte(byte);
    }
    hash.u64(candidate.requirement_identity.0 as u64);
    hash.u64(candidate.receipt.normalized_identity());
    hash.u64(candidate.bytes);
    hash.u64(candidate.alignment);
    hash.finish_or(Fnv1a::OFFSET_BASIS)
}

fn admitted_contribution_commitment(
    candidate: &SameStackContributionAdmissionCandidate,
) -> SameStackContributionCommitment {
    let mut hash =
        wire::digest_encoder::DomainDigestEncoder::new(b"omega.task.same-stack-contribution.v1\0");
    hash.u64(candidate.provider_plan_report_identity);
    hash.raw(&candidate.provider_plan_commitment.as_bytes());
    hash.raw(&candidate.requirement_identity.0.to_le_bytes());
    hash.raw(&candidate.receipt.normalized_identity().to_le_bytes());
    hash.u64(candidate.bytes);
    hash.u64(candidate.alignment);
    SameStackContributionCommitment::from_digest(hash.finish())
}

#[cfg(test)]
mod tests;
