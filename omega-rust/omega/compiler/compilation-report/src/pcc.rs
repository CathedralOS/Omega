//! Native proof-carrying product admission.
//!
//! Native semantics and correspondence evidence are not yet available in a
//! standalone profile. Producer custody digests, even when self-consistent,
//! do not prove anything about the behavior of the identified native bytes.

use terminal_codec::{
    PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy, PccRejection,
    PccVerificationOutcome, verify_pcc_claim_fields,
};

/// One published artifact/`.proof` companion pair with the exact separate
/// byte sizes the contract requires producer and receiver to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PccPublicationReceipt {
    pub product: PccProductKind,
    pub artifact_path: std::path::PathBuf,
    pub artifact_byte_len: u64,
    pub sidecar_path: std::path::PathBuf,
    pub sidecar_byte_len: u64,
}

impl std::fmt::Display for PccPublicationReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let product = match self.product {
            PccProductKind::Psi => "psi",
            PccProductKind::Native => "native",
        };
        write!(
            formatter,
            "{product} proof-carrying pair: {} ({} bytes) + {} ({} bytes)",
            self.artifact_path.display(),
            self.artifact_byte_len,
            self.sidecar_path.display(),
            self.sidecar_byte_len
        )
    }
}

/// Check the envelope of a native artifact/proof pair, without granting
/// native assurance until standalone semantics/preservation checking exists.
///
/// Claim-field validation still rejects invalid bindings or receiver-policy
/// violations. A valid envelope cannot establish native behavior, regardless
/// of the offered profile or its embedded Psi evidence.
pub fn verify_native_proof_sidecar(
    executable_bytes: &[u8],
    sidecar_bytes: &[u8],
    policy: &PccReceiverPolicy,
) -> PccVerificationOutcome {
    let sidecar = match verify_pcc_claim_fields(executable_bytes, sidecar_bytes, policy) {
        Ok(sidecar) => sidecar,
        Err(outcome) => return outcome,
    };
    if sidecar.product() != PccProductKind::Native {
        return PccVerificationOutcome::Reject(PccRejection::new(
            "product kind",
            "sidecar does not certify a native product",
        ));
    }
    PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
        product: PccProductKind::Native,
    })
}

/// Standalone receiver checking for one published artifact/`.proof` pair.
///
/// A receiver holds only the artifact bytes, the companion bytes and its own
/// pinned policy — never the source, producer memory, or producer hints about
/// which product the pair certifies. The envelope's declared product kind is
/// the only routing input: a Psi companion replays the bounded terminal
/// verification, while a native companion stays fail-closed `Incomplete`
/// until standalone semantics/preservation checking exists. A filename or the
/// artifact's own shape never selects the leg, so a Psi sidecar beside native
/// bytes still rejects inside the Psi leg and a relabeled envelope gains
/// nothing.
///
/// This decode exists only to read the declared kind. Each product leg
/// re-decodes the envelope and re-checks every claim field itself, so the
/// early decode trusts nothing the product checker would not independently
/// establish — and a malformed envelope rejects here before any product leg
/// runs.
pub fn verify_published_proof_pair(
    artifact_bytes: &[u8],
    sidecar_bytes: &[u8],
    policy: &PccReceiverPolicy,
) -> PccVerificationOutcome {
    let product = match PccProofSidecar::from_bytes(sidecar_bytes) {
        Ok(sidecar) => sidecar.product(),
        Err(error) => {
            return PccVerificationOutcome::Reject(PccRejection::new(
                "sidecar envelope",
                format!("invalid encoding: {error}"),
            ));
        }
    };
    match product {
        PccProductKind::Psi => {
            terminal_codec::verify_psi_proof_sidecar(artifact_bytes, sidecar_bytes, policy)
        }
        PccProductKind::Native => {
            verify_native_proof_sidecar(artifact_bytes, sidecar_bytes, policy)
        }
    }
}
