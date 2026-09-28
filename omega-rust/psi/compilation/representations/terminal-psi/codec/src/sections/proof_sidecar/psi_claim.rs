//! Reconstruct the exact Psi claim offered beside a canonical artifact.
//!
//! Receiver admission uses this same reconstruction after independent proof replay
//! to reject omitted assumptions, dependencies, or substituted profile identities.

use super::{
    PccDependency, PccGuarantee, PccProductKind, PccProofSidecar, pcc_artifact_commitment,
};
use crate::{
    CanonicalTerminalArtifact, CodecError, TrustDependencyStatus, current_terminal_trust_graph,
    decode_module, terminal_psi_identity,
};
use proof_admission::AdmissionProfile;
use sha2::{Digest, Sha256};

const ADMISSION_PROFILE_DOMAIN: &[u8] = b"omega.pcc.admission-profile.sha256.v1\0";

/// The bounded guarantee established by Terminal proof replay under the named profile.
pub const PSI_TERMINAL_VERIFIED_GUARANTEE: &str = "omega.terminal-verified-module.v1";

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

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("hex digit"));
        out.push(char::from_digit((byte & 0xf) as u32, 16).expect("hex digit"));
    }
    out
}
