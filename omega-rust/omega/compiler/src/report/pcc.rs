//! Published proof-pair receipts and routing to each product checker.

use native_artifact::verify_native_proof_sidecar;
use terminal_codec::{
    PccProductKind, PccProofSidecar, PccReceiverPolicy, PccRejection, PccVerificationOutcome,
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

/// Standalone receiver checking for one published artifact/`.proof` pair.
///
/// A receiver holds only the artifact bytes, the companion bytes and its own
/// pinned policy — never the source, producer memory, or producer hints about
/// which product the pair certifies. The envelope's declared product kind is
/// the only routing input: a Psi companion replays the bounded terminal
/// verification, while a native companion replays the checkable placed-image
/// leg and stays fail-closed `Incomplete` on the behavioral remainder. A
/// filename or the artifact's own shape never selects the leg, so a Psi
/// sidecar beside native bytes still rejects inside the Psi leg and a
/// relabeled envelope gains nothing.
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
