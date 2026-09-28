//! Canonical proof-sidecar claims and their wire envelope.
//!
//! This owner encodes, decodes, and checks canonical claim shape. `psi_claim`
//! reconstructs offered Psi claims; `crate::artifact_admission` owns receiver
//! policy and independent proof replay. Decoding a sidecar grants no authority.

pub(crate) mod psi_claim;

use crate::CodecError;
use crate::sections::semantic_module::wire::{Reader, Writer};
use sha2::{Digest, Sha256};

const PCC_MAGIC: &[u8; 8] = b"PCCPROOF";
const PCC_FORMAT_MARKER: u16 = 1;
pub(crate) const MAX_PCC_EVIDENCE_BYTES: usize = 1 << 26;
const MAX_PCC_CLAIM_COUNT: usize = 4_096;

const ARTIFACT_COMMITMENT_DOMAIN: &[u8] = b"omega.pcc.artifact-commitment.sha256.v1\0";

/// The product this `.proof` companion certifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PccProductKind {
    Psi,
    Native,
}

impl PccProductKind {
    fn encode(self) -> u8 {
        match self {
            Self::Psi => 1,
            Self::Native => 2,
        }
    }

    fn decode(tag: u8) -> Result<Self, CodecError> {
        match tag {
            1 => Ok(Self::Psi),
            2 => Ok(Self::Native),
            tag => Err(CodecError::InvalidTag("pcc product kind", tag)),
        }
    }
}

/// One offered guarantee and every premise it is conditioned on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccGuarantee {
    pub identity: String,
    pub premises: Vec<String>,
}

/// One omitted dependency the receiver must independently possess, validate
/// and accept, identified by exact semantic/content commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccDependency {
    pub identity: String,
    pub content_commitment: [u8; 32],
}

impl PccDependency {
    /// The exact content commitment of one installation-reach dependency: the
    /// canonical requirement identity plus its named upper-bound service
    /// closure. Module-local ServiceIds cannot identify receiver-held material.
    pub fn from_installation_reach(
        dependency: &terminal_psi::InstallationReachDependency,
        services: &[terminal_psi::ServiceDeclaration],
    ) -> Result<Self, CodecError> {
        let mut identities = dependency
            .upper_bound
            .iter()
            .map(|identity| {
                services
                    .iter()
                    .find(|service| service.id == *identity)
                    .map(|service| service.identity.as_str())
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "installation reach references an undeclared service",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        identities.sort_unstable();
        let mut digest = Sha256::new();
        digest.update(b"omega.pcc.installation-reach.sha256.v2\0");
        digest.update((dependency.requirement_identity.len() as u64).to_le_bytes());
        digest.update(dependency.requirement_identity.as_bytes());
        digest.update((identities.len() as u64).to_le_bytes());
        for identity in identities {
            digest.update((identity.len() as u64).to_le_bytes());
            digest.update(identity.as_bytes());
        }
        Ok(Self {
            identity: dependency.requirement_identity.clone(),
            content_commitment: digest.finalize().into(),
        })
    }
}

/// The complete `.proof` companion envelope for one published artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccProofSidecar {
    product: PccProductKind,
    artifact_commitment: [u8; 32],
    semantic_profile: String,
    checker_profile: String,
    guarantees: Vec<PccGuarantee>,
    evidence: Vec<u8>,
    assumptions: Vec<String>,
    dependencies: Vec<PccDependency>,
}

impl PccProofSidecar {
    /// Build one canonical sidecar value. Claim collections are sorted and
    /// must be duplicate-free and non-empty where required so the wire form
    /// has exactly one canonical encoding.
    pub fn new(
        product: PccProductKind,
        artifact_commitment: [u8; 32],
        semantic_profile: String,
        checker_profile: String,
        mut guarantees: Vec<PccGuarantee>,
        evidence: Vec<u8>,
        mut assumptions: Vec<String>,
        mut dependencies: Vec<PccDependency>,
    ) -> Result<Self, CodecError> {
        if semantic_profile.is_empty() || checker_profile.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "pcc sidecar profiles must be non-empty",
            ));
        }
        if guarantees.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "pcc sidecar must offer at least one guarantee",
            ));
        }
        if evidence.len() > MAX_PCC_EVIDENCE_BYTES {
            return Err(CodecError::CollectionTooLong("pcc sidecar evidence"));
        }
        guarantees.sort_by(|left, right| left.identity.cmp(&right.identity));
        for guarantee in &mut guarantees {
            guarantee.premises.sort();
        }
        assumptions.sort();
        dependencies.sort_by(|left, right| left.identity.cmp(&right.identity));
        check_canonical_claims(&guarantees, &assumptions, &dependencies)?;
        Ok(Self {
            product,
            artifact_commitment,
            semantic_profile,
            checker_profile,
            guarantees,
            evidence,
            assumptions,
            dependencies,
        })
    }

    pub const fn product(&self) -> PccProductKind {
        self.product
    }

    pub const fn artifact_commitment(&self) -> &[u8; 32] {
        &self.artifact_commitment
    }

    pub fn semantic_profile(&self) -> &str {
        &self.semantic_profile
    }

    pub fn checker_profile(&self) -> &str {
        &self.checker_profile
    }

    pub fn guarantees(&self) -> &[PccGuarantee] {
        &self.guarantees
    }

    pub fn evidence(&self) -> &[u8] {
        &self.evidence
    }

    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn dependencies(&self) -> &[PccDependency] {
        &self.dependencies
    }

    /// Serialize the envelope in its single canonical byte order.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        writer.bytes(PCC_MAGIC);
        writer.u16(PCC_FORMAT_MARKER);
        writer.u8(self.product.encode());
        writer.bytes(&self.artifact_commitment);
        writer
            .string("pcc semantic profile", &self.semantic_profile)
            .expect("profile identities are bounded by construction");
        writer
            .string("pcc checker profile", &self.checker_profile)
            .expect("profile identities are bounded by construction");
        writer
            .len("pcc guarantees", self.guarantees.len())
            .expect("claim count is bounded by construction");
        for guarantee in &self.guarantees {
            writer
                .string("pcc guarantee", &guarantee.identity)
                .expect("claim identity is bounded by construction");
            writer
                .len("pcc premises", guarantee.premises.len())
                .expect("claim count is bounded by construction");
            for premise in &guarantee.premises {
                writer
                    .string("pcc premise", premise)
                    .expect("claim identity is bounded by construction");
            }
        }
        writer.u64(u64::try_from(self.evidence.len()).expect("evidence length fits u64"));
        writer.bytes(&self.evidence);
        writer
            .strings("pcc assumptions", &self.assumptions)
            .expect("claim identities are bounded by construction");
        writer
            .len("pcc dependencies", self.dependencies.len())
            .expect("claim count is bounded by construction");
        for dependency in &self.dependencies {
            writer
                .string("pcc dependency", &dependency.identity)
                .expect("claim identity is bounded by construction");
            writer.bytes(&dependency.content_commitment);
        }
        writer.finish()
    }

    /// Decode one sidecar. The decoded value must re-encode to the exact
    /// input bytes, so non-canonical orderings, duplicates and trailing bytes
    /// all reject.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::new(bytes);
        if reader.take(PCC_MAGIC.len())? != PCC_MAGIC {
            return Err(CodecError::InvalidMagic);
        }
        let marker = reader.u16()?;
        if marker != PCC_FORMAT_MARKER {
            return Err(CodecError::UnsupportedFormatMarker(marker));
        }
        let product = PccProductKind::decode(reader.u8()?)?;
        let artifact_commitment: [u8; 32] = reader.array()?;
        let semantic_profile = reader.string("pcc semantic profile")?;
        let checker_profile = reader.string("pcc checker profile")?;
        if semantic_profile.is_empty() || checker_profile.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "pcc sidecar profiles must be non-empty",
            ));
        }
        let guarantee_count = reader.count()?;
        if guarantee_count == 0 || guarantee_count as usize > MAX_PCC_CLAIM_COUNT {
            return Err(CodecError::CollectionTooLong("pcc guarantees"));
        }
        let mut guarantees = Vec::with_capacity(guarantee_count as usize);
        for _ in 0..guarantee_count {
            let identity = reader.string("pcc guarantee")?;
            let premise_count = reader.count()?;
            if premise_count as usize > MAX_PCC_CLAIM_COUNT {
                return Err(CodecError::CollectionTooLong("pcc premises"));
            }
            let mut premises = Vec::with_capacity(premise_count as usize);
            for _ in 0..premise_count {
                premises.push(reader.string("pcc premise")?);
            }
            guarantees.push(PccGuarantee { identity, premises });
        }
        let evidence_len = usize::try_from(reader.u64()?)
            .map_err(|_| CodecError::CollectionTooLong("pcc sidecar evidence"))?;
        if evidence_len > MAX_PCC_EVIDENCE_BYTES {
            return Err(CodecError::CollectionTooLong("pcc sidecar evidence"));
        }
        let evidence = reader.take(evidence_len)?.to_vec();
        let assumptions = read_bounded_strings(&mut reader, "pcc assumptions")?;
        let dependency_count = reader.count()?;
        if dependency_count as usize > MAX_PCC_CLAIM_COUNT {
            return Err(CodecError::CollectionTooLong("pcc dependencies"));
        }
        let mut dependencies = Vec::with_capacity(dependency_count as usize);
        for _ in 0..dependency_count {
            dependencies.push(PccDependency {
                identity: reader.string("pcc dependency")?,
                content_commitment: reader.array()?,
            });
        }
        if reader.remaining() != 0 {
            return Err(CodecError::TrailingBytes(reader.remaining()));
        }
        let sidecar = Self {
            product,
            artifact_commitment,
            semantic_profile,
            checker_profile,
            guarantees,
            evidence,
            assumptions,
            dependencies,
        };
        check_canonical_claims(
            &sidecar.guarantees,
            &sidecar.assumptions,
            &sidecar.dependencies,
        )?;
        if sidecar.to_bytes() != bytes {
            return Err(CodecError::NonCanonicalEncoding);
        }
        Ok(sidecar)
    }
}

fn read_bounded_strings(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Vec<String>, CodecError> {
    let count = reader.count()?;
    if count as usize > MAX_PCC_CLAIM_COUNT {
        return Err(CodecError::CollectionTooLong(label));
    }
    (0..count).map(|_| reader.string(label)).collect()
}

/// The canonical claim invariants shared by construction and decode: strict
/// ordering, no duplicates, no empty identities.
fn check_canonical_claims(
    guarantees: &[PccGuarantee],
    assumptions: &[String],
    dependencies: &[PccDependency],
) -> Result<(), CodecError> {
    for window in guarantees.windows(2) {
        if window[0].identity >= window[1].identity {
            return Err(CodecError::NonCanonicalOrder("pcc guarantees"));
        }
    }
    for guarantee in guarantees {
        if guarantee.identity.is_empty() {
            return Err(CodecError::MalformedStructuralFoundation(
                "pcc guarantee identity must be non-empty",
            ));
        }
        for window in guarantee.premises.windows(2) {
            if window[0] >= window[1] {
                return Err(CodecError::NonCanonicalOrder("pcc premises"));
            }
        }
        if guarantee.premises.iter().any(String::is_empty) {
            return Err(CodecError::MalformedStructuralFoundation(
                "pcc premise must be non-empty",
            ));
        }
    }
    for window in assumptions.windows(2) {
        if window[0] >= window[1] {
            return Err(CodecError::NonCanonicalOrder("pcc assumptions"));
        }
    }
    if assumptions.iter().any(String::is_empty) {
        return Err(CodecError::MalformedStructuralFoundation(
            "pcc assumption must be non-empty",
        ));
    }
    for window in dependencies.windows(2) {
        if window[0].identity >= window[1].identity {
            return Err(CodecError::NonCanonicalOrder("pcc dependencies"));
        }
    }
    if dependencies
        .iter()
        .any(|dependency| dependency.identity.is_empty())
    {
        return Err(CodecError::MalformedStructuralFoundation(
            "pcc dependency identity must be non-empty",
        ));
    }
    Ok(())
}

/// The exact content commitment a sidecar claims for its artifact bytes.
pub fn pcc_artifact_commitment(artifact_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ARTIFACT_COMMITMENT_DOMAIN);
    digest.update((artifact_bytes.len() as u64).to_le_bytes());
    digest.update(artifact_bytes);
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use crate::{
        PccGuarantee, PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy,
        PccVerificationOutcome, admission_profile_identity, pcc_artifact_commitment,
        verify_pcc_claim_fields,
    };
    use proof_admission::AdmissionProfile;

    #[test]
    fn admission_profile_identity_is_order_canonical() {
        use proof_admission::AdmissionAcceptance;
        use semantic_vocabulary::{AdmissionSiteId, EvidenceIdentity, ProfileDecisionId};
        let acceptance = |n: u64| AdmissionAcceptance {
            site: AdmissionSiteId::new(n).expect("site"),
            evidence_identity: EvidenceIdentity::new(n + 1).expect("evidence"),
            profile_decision: ProfileDecisionId::new(n + 2).expect("decision"),
        };
        let forward = AdmissionProfile::from_acceptances([acceptance(10), acceptance(20)]);
        let reverse = AdmissionProfile::from_acceptances([acceptance(20), acceptance(10)]);
        assert_eq!(
            admission_profile_identity(&forward),
            admission_profile_identity(&reverse)
        );
        let empty = AdmissionProfile::default();
        assert_ne!(
            admission_profile_identity(&empty),
            admission_profile_identity(&forward)
        );
    }

    #[test]
    fn claim_fields_report_named_resource_limits() {
        let artifact = b"artifact bytes";
        let sidecar = PccProofSidecar::new(
            PccProductKind::Psi,
            pcc_artifact_commitment(artifact),
            "s".to_owned(),
            "c".to_owned(),
            vec![PccGuarantee {
                identity: "g".to_owned(),
                premises: Vec::new(),
            }],
            vec![1, 2, 3],
            Vec::new(),
            Vec::new(),
        )
        .expect("sidecar");
        let bytes = sidecar.to_bytes();
        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.max_artifact_bytes = 2;
        assert_eq!(
            verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err(),
            PccVerificationOutcome::Incomplete(PccIncompleteness::ArtifactBytes {
                actual: artifact.len() as u64,
                limit: 2,
            })
        );
        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.max_evidence_bytes = 2;
        assert_eq!(
            verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err(),
            PccVerificationOutcome::Incomplete(PccIncompleteness::EvidenceBytes {
                actual: 3,
                limit: 2,
            })
        );
    }
}
