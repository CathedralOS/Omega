//! Receiver admission of canonical artifact/proof pairs.
//!
//! `verify_psi_proof_sidecar` checks receiver policy, decodes the artifact, replays
//! its proof, and compares the complete independently reconstructed claim. Canonical
//! decoding alone does not issue an admission verdict.

use crate::sections::proof_sidecar::MAX_PCC_EVIDENCE_BYTES;
use crate::{
    CanonicalTerminalArtifact, PccDependency, PccProductKind, PccProofSidecar,
    TerminalObligationLedgerFingerprint, admission_profile_identity, build_psi_proof_sidecar,
    decode_mathematical_signature, decode_module, decode_proof_section_for,
    pcc_artifact_commitment, psi_semantic_profile_identity,
};
use proof_admission::{AdmissionAcceptance, AdmissionProfile};
use terminal_psi::TerminalPsiIdentity;

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
    // The mathematical-signature section, when carried, decodes into a fresh
    // arena and re-decides every declaration through the kernel — the
    // receiver replays it rather than trusting the producer's success.
    if let Some(signature_bytes) = artifact.mathematical_signature_bytes() {
        decode_mathematical_signature(signature_bytes).map_err(|error| {
            PccRejection::new(
                "psi mathematical signature section",
                format!("invalid canonical mathematical signature: {error}"),
            )
        })?;
    }
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
                .guarantees()
                .iter()
                .map(|guarantee| guarantee.identity.clone())
                .collect(),
            admitted_premises: sidecar
                .guarantees()
                .iter()
                .flat_map(|guarantee| guarantee.premises.iter().cloned())
                .collect(),
            accepted_semantic_profiles: vec![sidecar.semantic_profile().to_owned()],
            accepted_checker_profiles: vec![sidecar.checker_profile().to_owned()],
            admitted_assumptions: sidecar.assumptions().to_vec(),
            possessed_dependencies: sidecar.dependencies().to_vec(),
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
    if sidecar.evidence().len() as u64 > policy.max_evidence_bytes {
        return Err(PccVerificationOutcome::Incomplete(
            PccIncompleteness::EvidenceBytes {
                actual: sidecar.evidence().len() as u64,
                limit: policy.max_evidence_bytes,
            },
        ));
    }
    if pcc_artifact_commitment(artifact_bytes) != *sidecar.artifact_commitment() {
        return Err(PccVerificationOutcome::reject(
            "artifact bytes",
            "artifact content does not match the sidecar commitment",
        ));
    }
    if !policy
        .accepted_semantic_profiles
        .iter()
        .any(|accepted| accepted == sidecar.semantic_profile())
    {
        return Err(PccVerificationOutcome::reject(
            "semantic profile",
            format!(
                "offered semantic profile {} is not accepted by the receiver policy",
                sidecar.semantic_profile()
            ),
        ));
    }
    if !policy
        .accepted_checker_profiles
        .iter()
        .any(|accepted| accepted == sidecar.checker_profile())
    {
        return Err(PccVerificationOutcome::reject(
            "checker profile",
            format!(
                "offered checker profile {} is not accepted by the receiver policy",
                sidecar.checker_profile()
            ),
        ));
    }
    for required in &policy.required_guarantees {
        if !sidecar
            .guarantees()
            .iter()
            .any(|guarantee| &guarantee.identity == required)
        {
            return Err(PccVerificationOutcome::reject(
                "required guarantee",
                format!("offered claim does not cover required guarantee {required}"),
            ));
        }
    }
    for guarantee in sidecar.guarantees() {
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
    for assumption in sidecar.assumptions() {
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
    for dependency in sidecar.dependencies() {
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
    if sidecar.product() != PccProductKind::Psi {
        return PccVerificationOutcome::reject(
            "product kind",
            "sidecar does not certify a Psi product",
        );
    }
    if !sidecar.evidence().is_empty() {
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
            sidecar.semantic_profile() == established.semantic_profile(),
        ),
        (
            "checker profile",
            sidecar.checker_profile() == established.checker_profile(),
        ),
        (
            "guarantees",
            sidecar.guarantees() == established.guarantees(),
        ),
        (
            "assumption closure",
            sidecar.assumptions() == established.assumptions(),
        ),
        (
            "dependency inventory",
            sidecar.dependencies() == established.dependencies(),
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
        product: sidecar.product(),
        artifact_commitment: *sidecar.artifact_commitment(),
        semantic_subject: verdict.semantic_subject,
        obligation_ledger: verdict.obligation_ledger,
        semantic_profile: verdict.semantic_profile,
        checker_profile: verdict.checker_profile,
        admissions: verdict.admissions,
        policy_package_identity: policy.policy_package_identity.clone(),
        configuration_identity: policy.configuration_identity.clone(),
        accepted_guarantees: sidecar
            .guarantees()
            .iter()
            .map(|guarantee| guarantee.identity.clone())
            .collect(),
    })
}
