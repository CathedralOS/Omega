//! Native proof-carrying product admission.
//!
//! Native semantics and correspondence evidence are not yet available in a
//! standalone profile. Producer custody digests, even when self-consistent,
//! do not prove anything about the behavior of the identified native bytes.

use terminal_codec::{
    PccIncompleteness, PccProductKind, PccReceiverPolicy, PccRejection, PccVerificationOutcome,
    verify_pcc_claim_fields,
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
