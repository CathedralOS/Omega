//! Bounded PCC `.proof` sidecar envelope.
//!
//! A proof sidecar is a separate file appended to the complete artifact
//! filename (`<artifact>.proof`), never an embedded section. It carries an
//! explicit product kind, the exact artifact content commitment, semantic and
//! checker profile identities, the offered guarantees with all premises, the
//! required evidence bytes, the assumption closure and the exact dependency
//! inventory. A filename, producer signature or digest alone establishes
//! neither a claim nor permission to execute: receivers independently decode
//! the envelope, recompute the artifact commitment, replay the evidence and
//! check every offered claim against a receiver-owned policy.
//!
//! This module implements the bounded supported-evidence slice of
//! `wiki/spec/proofs/publication.md`. The Psi leg re-decodes the canonical
//! terminal artifact and re-runs `terminal_verifier::verify_module` under the
//! receiver's admission profile; the guarantee it can honestly offer is the
//! terminal verification guarantee under the toolchain's declared trust
//! closure. General PCC claims remain blocked on `PROOF-KERNEL-CORE`,
//! `PROOF-CERTIFICATION-BRIDGE` and the completed profile rules.

use proof_admission::{AdmissionAcceptance, AdmissionProfile};
use sha2::{Digest, Sha256};
use terminal_psi::TerminalPsiIdentity;

use crate::sections::semantic_module::wire::{Reader, Writer};
use crate::{
    CanonicalTerminalArtifact, CodecError, TerminalObligationLedgerFingerprint,
    TrustDependencyStatus, current_terminal_trust_graph, decode_module, decode_proof_section_for,
    terminal_psi_identity,
};

const PCC_MAGIC: &[u8; 8] = b"PCCPROOF";
const PCC_FORMAT_MARKER: u16 = 1;
const MAX_PCC_EVIDENCE_BYTES: usize = 1 << 26;
const MAX_PCC_CLAIM_COUNT: usize = 4_096;

const ARTIFACT_COMMITMENT_DOMAIN: &[u8] = b"omega.pcc.artifact-commitment.sha256.v1\0";
const ADMISSION_PROFILE_DOMAIN: &[u8] = b"omega.pcc.admission-profile.sha256.v1\0";

/// The bounded guarantee a Psi sidecar can offer today: the canonical
/// semantic module and proof bundle verify under the sidecar's checker
/// profile. Native custody, loading semantics and kernel-derived claims are
/// named by their own guarantee identities when their evidence lands.
pub const PSI_TERMINAL_VERIFIED_GUARANTEE: &str = "omega.terminal-verified-module.v1";

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

/// Stable checker-profile identity: a domain-separated digest of the complete
/// canonical acceptance set.
pub fn admission_profile_identity(profile: &AdmissionProfile) -> String {
    let mut digest = Sha256::new();
    digest.update(ADMISSION_PROFILE_DOMAIN);
    let mut count = 0u64;
    for acceptance in profile.acceptances() {
        digest.update(acceptance.site.get().to_le_bytes());
        digest.update(acceptance.evidence_identity.get().to_le_bytes());
        digest.update(acceptance.profile_decision.get().to_le_bytes());
        count += 1;
    }
    digest.update(count.to_le_bytes());
    let digest: [u8; 32] = digest.finalize().into();
    format!("proof-admission-v1:{}", hex_lower(&digest))
}

/// The semantic profile identity of one canonical terminal artifact: its
/// terminal vocabulary marker.
pub fn psi_semantic_profile_identity(
    artifact: &CanonicalTerminalArtifact,
) -> Result<String, CodecError> {
    let module = decode_module(artifact.semantic_bytes())?;
    let identity = terminal_psi_identity(&module)?;
    Ok(format!(
        "terminal-psi-vocabulary-{}",
        identity.vocabulary_marker.get()
    ))
}

/// The complete assumption closure the current toolchain's bounded
/// verification honestly relies on: every trust-graph node that is not yet
/// fully kernel-derived. Sorted for canonical encoding.
pub fn terminal_assumption_closure() -> Vec<String> {
    let graph = current_terminal_trust_graph().expect("built-in trust graph validates");
    let mut assumptions: Vec<String> = graph
        .nodes()
        .iter()
        .filter(|node| node.status() != TrustDependencyStatus::FullyDerived)
        .map(|node| node.identity().to_owned())
        .collect();
    assumptions.sort();
    assumptions
}

/// Build the bounded Psi sidecar for one canonical terminal artifact.
///
/// The offered guarantee is the terminal verification guarantee; the evidence
/// is empty because the Psi product's checkable evidence already lives in the
/// artifact's canonical semantic/proof sections. Assumptions name the current
/// trusted-judgment closure and dependencies enumerate the module's exact
/// installation-reach requirements.
pub fn build_psi_proof_sidecar(
    artifact: &CanonicalTerminalArtifact,
    profile: &AdmissionProfile,
    artifact_bytes: &[u8],
) -> Result<PccProofSidecar, CodecError> {
    let module = decode_module(artifact.semantic_bytes())?;
    let dependencies: Vec<PccDependency> = module
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| PccDependency::from_installation_reach(dependency, &module.services))
        .collect::<Result<_, _>>()?;
    PccProofSidecar::new(
        PccProductKind::Psi,
        pcc_artifact_commitment(artifact_bytes),
        psi_semantic_profile_identity(artifact)?,
        admission_profile_identity(profile),
        vec![PccGuarantee {
            identity: PSI_TERMINAL_VERIFIED_GUARANTEE.to_owned(),
            premises: Vec::new(),
        }],
        Vec::new(),
        terminal_assumption_closure(),
        dependencies,
    )
}

/// The subject-qualified result of independently replaying the terminal
/// verification leg of one decoded artifact: which semantic subject,
/// reconstructed obligation ledger, and admission profile the verdict rests
/// on. A verified product is never reported unqualified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalProofVerdict {
    /// The verifier-reconstructed semantic subject the proof was admitted for.
    pub semantic_subject: TerminalPsiIdentity,
    /// The reconstructed obligation-ledger identity the proof discharged.
    pub obligation_ledger: TerminalObligationLedgerFingerprint,
    /// The semantic profile independently established by replay.
    pub semantic_profile: String,
    /// The admission-profile identity the evidence replayed under.
    pub checker_profile: String,
    /// The receiver-profile admissions the verdict rests on.
    pub admissions: Vec<AdmissionAcceptance>,
}

/// Independently replay the terminal verification leg of one decoded
/// artifact: decode the canonical sections, require the proof section's seal
/// to name this exact reconstructed semantic subject, and re-run the verifier
/// under the receiver's admission profile. The returned verdict names the
/// qualified subject, ledger, and admissions the acceptance rests on.
pub fn verify_terminal_artifact_proof(
    artifact: &CanonicalTerminalArtifact,
    profile: &AdmissionProfile,
) -> Result<TerminalProofVerdict, PccRejection> {
    let module = decode_module(artifact.semantic_bytes()).map_err(|error| {
        PccRejection::new(
            "psi semantic section",
            format!("invalid canonical module: {error}"),
        )
    })?;
    let proof = decode_proof_section_for(&module, artifact.proof_bytes()).map_err(|error| {
        PccRejection::new(
            "psi proof section",
            format!("invalid canonical proof: {error}"),
        )
    })?;
    terminal_verifier::verify_module(&module, &proof, profile).map_err(|error| {
        PccRejection::new(
            "psi evidence",
            format!("terminal verification failed: {error}"),
        )
    })?;
    let manifest = artifact.manifest();
    let semantic_profile = psi_semantic_profile_identity(artifact).map_err(|error| {
        PccRejection::new(
            "psi semantic profile",
            format!("cannot reconstruct semantic profile: {error}"),
        )
    })?;
    Ok(TerminalProofVerdict {
        semantic_subject: manifest.semantic(),
        obligation_ledger: manifest.obligations(),
        semantic_profile,
        checker_profile: admission_profile_identity(profile),
        admissions: profile.acceptances().copied().collect(),
    })
}

/// The receiver-owned policy for proof-sidecar admission.
///
/// Every field is fixed by the receiver's independently pinned policy package
/// and concrete configuration; nothing here is taken from producer hints.
/// `policy_package_identity` and `configuration_identity` are reported back in
/// acceptance evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct PccReceiverPolicy {
    pub policy_package_identity: String,
    pub configuration_identity: String,
    /// Guarantee identities the offered claim must cover.
    pub required_guarantees: Vec<String>,
    /// Premise identities the receiver admits for any offered guarantee.
    pub admitted_premises: Vec<String>,
    /// Semantic-profile identities the receiver accepts.
    pub accepted_semantic_profiles: Vec<String>,
    /// Checker-profile identities the receiver accepts.
    pub accepted_checker_profiles: Vec<String>,
    /// Assumption identities the receiver permits for these claim roles.
    pub admitted_assumptions: Vec<String>,
    /// Dependency material the receiver independently possesses, validated.
    pub possessed_dependencies: Vec<PccDependency>,
    /// The receiver's admission profile used to replay checkable evidence.
    pub admission_profile: AdmissionProfile,
    /// Resource bound on accepted artifact bytes.
    pub max_artifact_bytes: u64,
    /// Resource bound on accepted sidecar evidence bytes.
    pub max_evidence_bytes: u64,
}

impl PccReceiverPolicy {
    /// The producer-side validation policy: accept exactly the claim the
    /// sidecar offers. Publication stages and validates every pair with this
    /// self-consistent policy before reporting success; it confers no
    /// receiver authority.
    pub fn for_offered_claim(sidecar: &PccProofSidecar, profile: AdmissionProfile) -> Self {
        Self {
            policy_package_identity: "producer-self-check".to_owned(),
            configuration_identity: "offered-claim".to_owned(),
            required_guarantees: sidecar
                .guarantees
                .iter()
                .map(|guarantee| guarantee.identity.clone())
                .collect(),
            admitted_premises: sidecar
                .guarantees
                .iter()
                .flat_map(|guarantee| guarantee.premises.iter().cloned())
                .collect(),
            accepted_semantic_profiles: vec![sidecar.semantic_profile.clone()],
            accepted_checker_profiles: vec![sidecar.checker_profile.clone()],
            admitted_assumptions: sidecar.assumptions.clone(),
            possessed_dependencies: sidecar.dependencies.clone(),
            admission_profile: profile,
            max_artifact_bytes: u64::MAX,
            max_evidence_bytes: MAX_PCC_EVIDENCE_BYTES as u64,
        }
    }
}

/// One verified artifact/proof pair acceptance. The verdict is always
/// qualified: it names the exact reconstructed semantic subject, the
/// reconstructed obligation ledger, the semantic and checker profiles, and
/// the disclosed admissions the acceptance rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccVerifiedProduct {
    pub product: PccProductKind,
    pub artifact_commitment: [u8; 32],
    /// The verifier-reconstructed semantic subject the proof was admitted for.
    pub semantic_subject: TerminalPsiIdentity,
    /// The reconstructed obligation-ledger identity the proof discharged.
    pub obligation_ledger: TerminalObligationLedgerFingerprint,
    /// The semantic profile independently established by replay.
    pub semantic_profile: String,
    /// The admission/checker profile identity the evidence replayed under.
    pub checker_profile: String,
    /// The disclosed receiver-profile admissions the verdict rests on.
    pub admissions: Vec<AdmissionAcceptance>,
    pub policy_package_identity: String,
    pub configuration_identity: String,
    pub accepted_guarantees: Vec<String>,
}

/// A named failed subject with its reason, per the contract's Reject outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccRejection {
    pub subject: String,
    pub reason: String,
}

impl PccRejection {
    pub fn new(subject: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            reason: reason.into(),
        }
    }
}

/// A resource limit or unsupported evidence, per the contract's Incomplete outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PccIncompleteness {
    ArtifactBytes { actual: u64, limit: u64 },
    EvidenceBytes { actual: u64, limit: u64 },
    UnsupportedEvidence { product: PccProductKind },
}

/// The three-way verification outcome the contract fixes: complete verified
/// pair, named rejection, or named incompleteness. There is no partial or
/// uncertified success.
#[derive(Debug, Clone, PartialEq)]
pub enum PccVerificationOutcome {
    Complete(PccVerifiedProduct),
    Reject(PccRejection),
    Incomplete(PccIncompleteness),
}

impl PccVerificationOutcome {
    fn reject(subject: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Reject(PccRejection::new(subject, reason))
    }
}

/// Check every product-independent claim in one decoded sidecar against the
/// artifact bytes and the receiver policy: exact content commitment, accepted
/// profiles, required guarantees with all premises admitted, assumption
/// closure permitted and every declared dependency possessed. The decoded
/// sidecar is returned for the caller's product-specific evidence leg.
pub fn verify_pcc_claim_fields(
    artifact_bytes: &[u8],
    sidecar_bytes: &[u8],
    policy: &PccReceiverPolicy,
) -> Result<PccProofSidecar, PccVerificationOutcome> {
    if artifact_bytes.len() as u64 > policy.max_artifact_bytes {
        return Err(PccVerificationOutcome::Incomplete(
            PccIncompleteness::ArtifactBytes {
                actual: artifact_bytes.len() as u64,
                limit: policy.max_artifact_bytes,
            },
        ));
    }
    let sidecar = PccProofSidecar::from_bytes(sidecar_bytes).map_err(|error| {
        PccVerificationOutcome::reject("sidecar envelope", format!("invalid encoding: {error}"))
    })?;
    if sidecar.evidence.len() as u64 > policy.max_evidence_bytes {
        return Err(PccVerificationOutcome::Incomplete(
            PccIncompleteness::EvidenceBytes {
                actual: sidecar.evidence.len() as u64,
                limit: policy.max_evidence_bytes,
            },
        ));
    }
    if pcc_artifact_commitment(artifact_bytes) != sidecar.artifact_commitment {
        return Err(PccVerificationOutcome::reject(
            "artifact bytes",
            "artifact content does not match the sidecar commitment",
        ));
    }
    if !policy
        .accepted_semantic_profiles
        .iter()
        .any(|accepted| accepted == &sidecar.semantic_profile)
    {
        return Err(PccVerificationOutcome::reject(
            "semantic profile",
            format!(
                "offered semantic profile {} is not accepted by the receiver policy",
                sidecar.semantic_profile
            ),
        ));
    }
    if !policy
        .accepted_checker_profiles
        .iter()
        .any(|accepted| accepted == &sidecar.checker_profile)
    {
        return Err(PccVerificationOutcome::reject(
            "checker profile",
            format!(
                "offered checker profile {} is not accepted by the receiver policy",
                sidecar.checker_profile
            ),
        ));
    }
    for required in &policy.required_guarantees {
        if !sidecar
            .guarantees
            .iter()
            .any(|guarantee| &guarantee.identity == required)
        {
            return Err(PccVerificationOutcome::reject(
                "required guarantee",
                format!("offered claim does not cover required guarantee {required}"),
            ));
        }
    }
    for guarantee in &sidecar.guarantees {
        for premise in &guarantee.premises {
            if !policy.admitted_premises.iter().any(|p| p == premise) {
                return Err(PccVerificationOutcome::reject(
                    "guarantee premise",
                    format!(
                        "guarantee {} is conditioned on premise {premise} the policy does not admit",
                        guarantee.identity
                    ),
                ));
            }
        }
    }
    for assumption in &sidecar.assumptions {
        if !policy
            .admitted_assumptions
            .iter()
            .any(|admitted| admitted == assumption)
        {
            return Err(PccVerificationOutcome::reject(
                "assumption",
                format!("assumption {assumption} is not permitted by the receiver policy"),
            ));
        }
    }
    for dependency in &sidecar.dependencies {
        if !policy
            .possessed_dependencies
            .iter()
            .any(|possessed| possessed == dependency)
        {
            return Err(PccVerificationOutcome::reject(
                "dependency",
                format!(
                    "omitted dependency {} is not independently possessed by the receiver",
                    dependency.identity
                ),
            ));
        }
    }
    Ok(sidecar)
}

/// Independently verify one Psi artifact/proof pair.
///
/// The receiver needs only the artifact bytes, the sidecar bytes and its own
/// pinned policy. The claim fields are checked first; the artifact's canonical
/// sections are then re-decoded and the terminal verifier replays every
/// obligation under the receiver's admission profile. The Psi product's
/// bounded contract requires an empty sidecar evidence field: all checkable
/// evidence already lives inside the artifact.
pub fn verify_psi_proof_sidecar(
    artifact_bytes: &[u8],
    sidecar_bytes: &[u8],
    policy: &PccReceiverPolicy,
) -> PccVerificationOutcome {
    let sidecar = match verify_pcc_claim_fields(artifact_bytes, sidecar_bytes, policy) {
        Ok(sidecar) => sidecar,
        Err(outcome) => return outcome,
    };
    if sidecar.product != PccProductKind::Psi {
        return PccVerificationOutcome::reject(
            "product kind",
            "sidecar does not certify a Psi product",
        );
    }
    if !sidecar.evidence.is_empty() {
        return PccVerificationOutcome::reject(
            "psi evidence",
            "bounded Psi sidecar evidence must be empty; evidence lives in the artifact",
        );
    }
    let artifact = match CanonicalTerminalArtifact::from_bytes(artifact_bytes) {
        Ok(artifact) => artifact,
        Err(error) => {
            return PccVerificationOutcome::reject(
                "psi artifact",
                format!("artifact is not a canonical terminal artifact: {error}"),
            );
        }
    };
    let verdict = match verify_terminal_artifact_proof(&artifact, &policy.admission_profile) {
        Ok(verdict) => verdict,
        Err(rejection) => return PccVerificationOutcome::Reject(rejection),
    };
    // Replay establishes this exact bounded claim, not arbitrary labels offered
    // by the producer. Reconstruct its profiles and full trust/dependency closure
    // independently, including entries the sidecar might have omitted.
    let established =
        match build_psi_proof_sidecar(&artifact, &policy.admission_profile, artifact_bytes) {
            Ok(established) => established,
            Err(error) => {
                return PccVerificationOutcome::reject(
                    "psi claim reconstruction",
                    format!("cannot reconstruct the verified claim: {error}"),
                );
            }
        };
    for (subject, matches) in [
        (
            "semantic profile",
            sidecar.semantic_profile == established.semantic_profile,
        ),
        (
            "checker profile",
            sidecar.checker_profile == established.checker_profile,
        ),
        ("guarantees", sidecar.guarantees == established.guarantees),
        (
            "assumption closure",
            sidecar.assumptions == established.assumptions,
        ),
        (
            "dependency inventory",
            sidecar.dependencies == established.dependencies,
        ),
    ] {
        if !matches {
            return PccVerificationOutcome::reject(
                subject,
                "offered claim differs from the claim independently established by Psi verification",
            );
        }
    }
    PccVerificationOutcome::Complete(PccVerifiedProduct {
        product: sidecar.product,
        artifact_commitment: sidecar.artifact_commitment,
        semantic_subject: verdict.semantic_subject,
        obligation_ledger: verdict.obligation_ledger,
        semantic_profile: verdict.semantic_profile,
        checker_profile: verdict.checker_profile,
        admissions: verdict.admissions,
        policy_package_identity: policy.policy_package_identity.clone(),
        configuration_identity: policy.configuration_identity.clone(),
        accepted_guarantees: sidecar
            .guarantees
            .iter()
            .map(|guarantee| guarantee.identity.clone())
            .collect(),
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("hex digit"));
        out.push(char::from_digit((byte & 0xf) as u32, 16).expect("hex digit"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        AdmissionProfile, CodecError, PSI_TERMINAL_VERIFIED_GUARANTEE, PccDependency, PccGuarantee,
        PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy,
        PccVerificationOutcome, admission_profile_identity, pcc_artifact_commitment,
        verify_pcc_claim_fields, verify_psi_proof_sidecar,
    };

    fn sample_sidecar() -> PccProofSidecar {
        PccProofSidecar::new(
            PccProductKind::Psi,
            [7; 32],
            "terminal-psi-vocabulary-96".to_owned(),
            "proof-admission-v1:00".to_owned(),
            vec![PccGuarantee {
                identity: PSI_TERMINAL_VERIFIED_GUARANTEE.to_owned(),
                premises: vec!["premise-a".to_owned()],
            }],
            Vec::new(),
            vec!["assumption-a".to_owned()],
            vec![PccDependency {
                identity: "dep-a".to_owned(),
                content_commitment: [9; 32],
            }],
        )
        .expect("sample sidecar")
    }

    #[test]
    fn sidecar_round_trips_canonically() {
        let sidecar = sample_sidecar();
        let bytes = sidecar.to_bytes();
        assert_eq!(
            PccProofSidecar::from_bytes(&bytes).expect("decode"),
            sidecar
        );
    }

    #[test]
    fn decode_rejects_unsorted_guarantees() {
        let mut sidecar = sample_sidecar();
        sidecar.guarantees.push(PccGuarantee {
            identity: "a-guarantee".to_owned(),
            premises: Vec::new(),
        });
        let bytes = sidecar.to_bytes();
        assert_eq!(
            PccProofSidecar::from_bytes(&bytes),
            Err(CodecError::NonCanonicalOrder("pcc guarantees"))
        );
    }

    #[test]
    fn decode_rejects_trailing_bytes_and_bad_magic() {
        let mut bytes = sample_sidecar().to_bytes();
        bytes.push(0);
        assert!(matches!(
            PccProofSidecar::from_bytes(&bytes),
            Err(CodecError::TrailingBytes(1))
        ));
        let mut bad = sample_sidecar().to_bytes();
        bad[0] = b'X';
        assert_eq!(
            PccProofSidecar::from_bytes(&bad),
            Err(CodecError::InvalidMagic)
        );
    }

    #[test]
    fn construction_sorts_claim_collections() {
        let sidecar = PccProofSidecar::new(
            PccProductKind::Native,
            [0; 32],
            "s".to_owned(),
            "c".to_owned(),
            vec![
                PccGuarantee {
                    identity: "b".to_owned(),
                    premises: vec!["p2".to_owned(), "p1".to_owned()],
                },
                PccGuarantee {
                    identity: "a".to_owned(),
                    premises: Vec::new(),
                },
            ],
            Vec::new(),
            vec!["z".to_owned(), "a".to_owned()],
            Vec::new(),
        )
        .expect("constructed");
        assert_eq!(sidecar.guarantees[0].identity, "a");
        assert_eq!(sidecar.guarantees[1].premises, ["p1", "p2"]);
        assert_eq!(sidecar.assumptions, ["a", "z"]);
    }

    #[test]
    fn construction_rejects_duplicates_and_empties() {
        assert!(
            PccProofSidecar::new(
                PccProductKind::Psi,
                [0; 32],
                "s".to_owned(),
                "c".to_owned(),
                vec![
                    PccGuarantee {
                        identity: "a".to_owned(),
                        premises: Vec::new()
                    },
                    PccGuarantee {
                        identity: "a".to_owned(),
                        premises: Vec::new()
                    },
                ],
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .is_err()
        );
        assert!(
            PccProofSidecar::new(
                PccProductKind::Psi,
                [0; 32],
                "s".to_owned(),
                "c".to_owned(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .is_err()
        );
    }

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

    fn rejecting_subject(outcome: PccVerificationOutcome) -> String {
        match outcome {
            PccVerificationOutcome::Reject(rejection) => rejection.subject,
            other => panic!("expected a named rejection, got {other:?}"),
        }
    }

    #[test]
    fn claim_fields_reject_wrong_artifact_bytes() {
        let artifact = b"the exact artifact bytes";
        let sidecar = PccProofSidecar::new(
            PccProductKind::Psi,
            pcc_artifact_commitment(artifact),
            "s".to_owned(),
            "c".to_owned(),
            vec![PccGuarantee {
                identity: "g".to_owned(),
                premises: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("sidecar");
        let policy = PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        assert_eq!(
            rejecting_subject(
                verify_pcc_claim_fields(
                    b"stale or substituted bytes",
                    &sidecar.to_bytes(),
                    &policy
                )
                .expect_err("stale sidecar binding must reject")
            ),
            "artifact bytes"
        );
    }

    #[test]
    fn claim_fields_reject_unaccepted_profiles_guarantees_premises_assumptions_and_dependencies() {
        let artifact = b"artifact";
        let sidecar = PccProofSidecar::new(
            PccProductKind::Psi,
            pcc_artifact_commitment(artifact),
            "sem-a".to_owned(),
            "check-a".to_owned(),
            vec![PccGuarantee {
                identity: "g-offered".to_owned(),
                premises: vec!["p-needed".to_owned()],
            }],
            Vec::new(),
            vec!["a-claimed".to_owned()],
            vec![PccDependency {
                identity: "d-needed".to_owned(),
                content_commitment: [1; 32],
            }],
        )
        .expect("sidecar");
        let bytes = sidecar.to_bytes();
        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        assert!(verify_pcc_claim_fields(artifact, &bytes, &policy).is_ok());

        policy.accepted_semantic_profiles = vec!["other".to_owned()];
        assert_eq!(
            rejecting_subject(verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err()),
            "semantic profile"
        );

        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.accepted_checker_profiles = vec!["other".to_owned()];
        assert_eq!(
            rejecting_subject(verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err()),
            "checker profile"
        );

        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.required_guarantees = vec!["g-required".to_owned()];
        assert_eq!(
            rejecting_subject(verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err()),
            "required guarantee"
        );

        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.admitted_premises = Vec::new();
        assert_eq!(
            rejecting_subject(verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err()),
            "guarantee premise"
        );

        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.admitted_assumptions = Vec::new();
        assert_eq!(
            rejecting_subject(verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err()),
            "assumption"
        );

        let mut policy =
            PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        policy.possessed_dependencies = Vec::new();
        assert_eq!(
            rejecting_subject(verify_pcc_claim_fields(artifact, &bytes, &policy).unwrap_err()),
            "dependency"
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

    #[test]
    fn psi_verification_rejects_wrong_product_kind_and_invalid_artifact() {
        let artifact = b"not a canonical artifact";
        let sidecar = PccProofSidecar::new(
            PccProductKind::Native,
            pcc_artifact_commitment(artifact),
            "s".to_owned(),
            "c".to_owned(),
            vec![PccGuarantee {
                identity: "g".to_owned(),
                premises: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("sidecar");
        let policy = PccReceiverPolicy::for_offered_claim(&sidecar, AdmissionProfile::default());
        assert_eq!(
            rejecting_subject(verify_psi_proof_sidecar(
                artifact,
                &sidecar.to_bytes(),
                &policy
            )),
            "product kind"
        );
        let psi_sidecar = PccProofSidecar::new(
            PccProductKind::Psi,
            pcc_artifact_commitment(artifact),
            "s".to_owned(),
            "c".to_owned(),
            vec![PccGuarantee {
                identity: "g".to_owned(),
                premises: Vec::new(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("sidecar");
        assert_eq!(
            rejecting_subject(verify_psi_proof_sidecar(
                artifact,
                &psi_sidecar.to_bytes(),
                &PccReceiverPolicy::for_offered_claim(&psi_sidecar, AdmissionProfile::default())
            )),
            "psi artifact"
        );
    }
}
