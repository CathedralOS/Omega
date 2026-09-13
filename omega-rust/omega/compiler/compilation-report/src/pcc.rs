//! Bounded native PCC custody evidence and independent verification.
//!
//! The native `.proof` sidecar carries a custody record that lets a receiver
//! recompute the exact publication certificate chain from carried fields: the
//! native artifact identity, the canonical Psi artifact it was realized from,
//! the target, image-symbol and validation digests, and the claimed
//! certificate/evidence digests. The verifier recomputes both digests —
//! including the container digest of the exact published bytes — and replays
//! the embedded Psi artifact's own terminal verification under the receiver's
//! admission profile. This is the bounded custody guarantee named
//! [`terminal_codec::NATIVE_CERTIFIED_CUSTODY_GUARANTEE`]; it does not claim
//! loaded-image semantics, which remain with `PROOF-CERTIFICATION-BRIDGE`.

use terminal_codec::{
    CanonicalTerminalArtifact, NATIVE_CERTIFIED_CUSTODY_GUARANTEE, PccDependency, PccGuarantee,
    PccProductKind, PccProofSidecar, PccReceiverPolicy, PccRejection, PccVerificationOutcome,
    PccVerifiedProduct, admission_profile_identity, pcc_artifact_commitment,
    psi_semantic_profile_identity, terminal_assumption_closure, verify_pcc_claim_fields,
    verify_terminal_artifact_proof,
};

use crate::{
    RetainedNativeArtifact, executable_container_digest, native_publication_certificate_digest,
    native_publication_evidence_digest,
};

const CUSTODY_EVIDENCE_VERSION: u16 = 1;

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

/// The exact custody fields a native sidecar carries so the receiver can
/// recompute the publication certificate chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePccCustodyEvidence {
    /// The complete canonical Psi artifact the native output was realized
    /// from. The standalone contract embeds it: a digest of missing Psi
    /// material is not a substitute for a checkable argument.
    pub psi_artifact_bytes: Vec<u8>,
    pub native_artifact_identity: [u8; 32],
    pub target: target::NativeTarget,
    pub final_image_symbol_digest: [u8; 32],
    pub boundary_contract_report_fingerprint: Option<u64>,
    pub text_validation_digest: [u8; 32],
    pub function_validation_digest: [u8; 32],
    pub function_validation_report_fingerprint: u64,
    pub inventory_digest: [u8; 32],
    pub inventory_report_fingerprint: u64,
    pub callback_placement_identity_report_fingerprint: u64,
    /// The claimed publication certificate the verifier must recompute.
    pub certificate_digest: [u8; 32],
    /// The claimed publication evidence digest the verifier must recompute.
    pub evidence_digest: [u8; 32],
}

impl NativePccCustodyEvidence {
    /// Capture the complete custody record for one retained native artifact.
    /// Requires the same strong validation evidence publication requires.
    pub fn from_artifact(artifact: &RetainedNativeArtifact) -> Result<Self, &'static str> {
        let output = artifact.image().output();
        let text_validation_digest = output
            .compiler_text_validation
            .map(|validation| validation.derivation_digest)
            .ok_or("native PCC custody requires strong compiler-text validation evidence")?;
        let function_validation = output
            .compiler_function_validation
            .ok_or("native PCC custody requires compiler-function validation evidence")?;
        let native_artifact_identity = *artifact.identity().as_bytes();
        let certificate_digest = native_publication_certificate_digest(
            &native_artifact_identity,
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            artifact.target(),
            artifact.image().final_image_symbol_digest(),
            function_validation.boundary_contract_report_fingerprint,
            text_validation_digest,
            function_validation.evidence_digest(),
            function_validation.evidence_report_fingerprint(),
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
        );
        let evidence_digest = native_publication_evidence_digest(
            &native_artifact_identity,
            certificate_digest,
            output.callback_placement_identity_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            text_validation_digest,
            function_validation.evidence_digest(),
            function_validation.evidence_report_fingerprint(),
            output.bytes.len(),
            executable_container_digest(&output.bytes),
        );
        Ok(Self {
            psi_artifact_bytes: artifact.psi_artifact().to_bytes(),
            native_artifact_identity,
            target: artifact.target(),
            final_image_symbol_digest: *artifact.image().final_image_symbol_digest().as_bytes(),
            boundary_contract_report_fingerprint: function_validation
                .boundary_contract_report_fingerprint,
            text_validation_digest: *text_validation_digest.as_bytes(),
            function_validation_digest: *function_validation.evidence_digest().as_bytes(),
            function_validation_report_fingerprint: function_validation
                .evidence_report_fingerprint(),
            inventory_digest: *output.executable_regions.inventory_digest.as_bytes(),
            inventory_report_fingerprint: output.executable_regions.inventory_report_fingerprint,
            callback_placement_identity_report_fingerprint: output
                .callback_placement_identity_report_fingerprint,
            certificate_digest: *certificate_digest.as_bytes(),
            evidence_digest: *evidence_digest.as_bytes(),
        })
    }

    /// Serialize the custody record in its single canonical byte order.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(192 + self.psi_artifact_bytes.len());
        bytes.extend_from_slice(&CUSTODY_EVIDENCE_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(self.psi_artifact_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&self.psi_artifact_bytes);
        bytes.extend_from_slice(&self.native_artifact_identity);
        bytes.push(match self.target.architecture {
            target::Architecture::Aarch64 => 1,
            target::Architecture::X86_64 => 2,
        });
        bytes.push(match self.target.object_format {
            target::ObjectFormat::Elf => 1,
            target::ObjectFormat::MachO => 2,
            target::ObjectFormat::Coff => 3,
        });
        bytes.extend_from_slice(&(self.target.pointer_size as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.target.pointer_alignment as u64).to_le_bytes());
        bytes.extend_from_slice(&self.final_image_symbol_digest);
        bytes.push(u8::from(
            self.boundary_contract_report_fingerprint.is_some(),
        ));
        bytes.extend_from_slice(
            &self
                .boundary_contract_report_fingerprint
                .unwrap_or_default()
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&self.text_validation_digest);
        bytes.extend_from_slice(&self.function_validation_digest);
        bytes.extend_from_slice(&self.function_validation_report_fingerprint.to_le_bytes());
        bytes.extend_from_slice(&self.inventory_digest);
        bytes.extend_from_slice(&self.inventory_report_fingerprint.to_le_bytes());
        bytes.extend_from_slice(
            &self
                .callback_placement_identity_report_fingerprint
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&self.certificate_digest);
        bytes.extend_from_slice(&self.evidence_digest);
        bytes
    }

    /// Decode one custody record, rejecting truncation, unknown versions and
    /// trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        let mut cursor = CustodyCursor::new(bytes);
        if cursor.u16()? != CUSTODY_EVIDENCE_VERSION {
            return Err("unsupported native custody evidence version");
        }
        let psi_len = usize::try_from(cursor.u64()?)
            .map_err(|_| "native custody psi artifact length overflows usize")?;
        let psi_artifact_bytes = cursor.take(psi_len)?.to_vec();
        let native_artifact_identity = cursor.array()?;
        let architecture = match cursor.u8()? {
            1 => target::Architecture::Aarch64,
            2 => target::Architecture::X86_64,
            _ => return Err("native custody target architecture tag is unknown"),
        };
        let object_format = match cursor.u8()? {
            1 => target::ObjectFormat::Elf,
            2 => target::ObjectFormat::MachO,
            3 => target::ObjectFormat::Coff,
            _ => return Err("native custody object format tag is unknown"),
        };
        let pointer_size = usize::try_from(cursor.u64()?)
            .map_err(|_| "native custody pointer size overflows usize")?;
        let pointer_alignment = usize::try_from(cursor.u64()?)
            .map_err(|_| "native custody pointer alignment overflows usize")?;
        let final_image_symbol_digest = cursor.array()?;
        let boundary_contract_report_fingerprint = match cursor.u8()? {
            0 => {
                let _ = cursor.u64()?;
                None
            }
            1 => Some(cursor.u64()?),
            _ => return Err("native custody boundary fingerprint tag is invalid"),
        };
        let evidence = Self {
            psi_artifact_bytes,
            native_artifact_identity,
            target: target::NativeTarget {
                architecture,
                object_format,
                pointer_size,
                pointer_alignment,
            },
            final_image_symbol_digest,
            boundary_contract_report_fingerprint,
            text_validation_digest: cursor.array()?,
            function_validation_digest: cursor.array()?,
            function_validation_report_fingerprint: cursor.u64()?,
            inventory_digest: cursor.array()?,
            inventory_report_fingerprint: cursor.u64()?,
            callback_placement_identity_report_fingerprint: cursor.u64()?,
            certificate_digest: cursor.array()?,
            evidence_digest: cursor.array()?,
        };
        if cursor.remaining() != 0 {
            return Err("native custody evidence has trailing bytes");
        }
        Ok(evidence)
    }

    /// The complete dependency inventory the embedded Psi artifact omits to
    /// the receiver: its installation-reach requirements, exactly identified.
    pub fn dependency_inventory(&self) -> Result<Vec<PccDependency>, &'static str> {
        let artifact = CanonicalTerminalArtifact::from_bytes(&self.psi_artifact_bytes)
            .map_err(|_| "native custody embeds an invalid canonical Psi artifact")?;
        let module = terminal_codec::decode_module(artifact.semantic_bytes())
            .map_err(|_| "native custody embeds an invalid canonical module")?;
        Ok(module
            .root_service_reach
            .installation_dependencies
            .iter()
            .map(PccDependency::from_installation_reach)
            .collect())
    }
}

/// Build the bounded native `.proof` sidecar for one retained artifact and
/// its exact publication bytes. The bytes are the post-finalization output;
/// any later byte change invalidates the sidecar's artifact commitment.
pub fn build_native_proof_sidecar(
    artifact: &RetainedNativeArtifact,
    executable_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<PccProofSidecar, String> {
    let custody = NativePccCustodyEvidence::from_artifact(artifact).map_err(str::to_owned)?;
    let dependencies = custody.dependency_inventory().map_err(str::to_owned)?;
    let psi_profile = psi_semantic_profile_identity(artifact.psi_artifact())
        .map_err(|error| format!("cannot derive the psi semantic profile: {error}"))?;
    PccProofSidecar::new(
        PccProductKind::Native,
        pcc_artifact_commitment(executable_bytes),
        format!("native-custody-v1/{psi_profile}"),
        admission_profile_identity(profile),
        vec![PccGuarantee {
            identity: NATIVE_CERTIFIED_CUSTODY_GUARANTEE.to_owned(),
            premises: Vec::new(),
        }],
        custody.to_bytes(),
        terminal_assumption_closure(),
        dependencies,
    )
    .map_err(|error| format!("cannot encode the native proof sidecar: {error}"))
}

/// Independently verify one native artifact/proof pair.
///
/// The receiver holds only the executable bytes, the `.proof` bytes and its
/// pinned policy — source, producer memory and separate Psi products are not
/// consulted. After the shared claim checks, the custody record is decoded,
/// the publication certificate chain is recomputed (including the container
/// digest of the exact executable bytes), and the embedded Psi artifact is
/// re-decoded and re-verified under the receiver's admission profile.
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
        return reject("product kind", "sidecar does not certify a native product");
    }
    let custody = match NativePccCustodyEvidence::from_bytes(sidecar.evidence()) {
        Ok(custody) => custody,
        Err(reason) => return reject("native custody evidence", reason),
    };
    let psi_artifact = match CanonicalTerminalArtifact::from_bytes(&custody.psi_artifact_bytes) {
        Ok(artifact) => artifact,
        Err(error) => {
            return reject(
                "native custody psi artifact",
                format!("embedded psi artifact is not canonical: {error}"),
            );
        }
    };
    let certificate = native_publication_certificate_digest(
        &custody.native_artifact_identity,
        psi_artifact.semantic_bytes(),
        psi_artifact.proof_bytes(),
        custody.target,
        image::FinalImageSymbolDigest::from_digest(custody.final_image_symbol_digest),
        custody.boundary_contract_report_fingerprint,
        image::CompilerTextDerivationDigest::from_digest(custody.text_validation_digest),
        image::CompilerFunctionValidationDigest::from_digest(custody.function_validation_digest),
        custody.function_validation_report_fingerprint,
        image::PlacedExecutableRegionInventoryDigest::from_digest(custody.inventory_digest),
        custody.inventory_report_fingerprint,
    );
    if certificate.as_bytes() != &custody.certificate_digest {
        return reject(
            "native custody certificate",
            "recomputed publication certificate does not match the claimed digest",
        );
    }
    let evidence = native_publication_evidence_digest(
        &custody.native_artifact_identity,
        certificate,
        custody.callback_placement_identity_report_fingerprint,
        image::PlacedExecutableRegionInventoryDigest::from_digest(custody.inventory_digest),
        custody.inventory_report_fingerprint,
        image::CompilerTextDerivationDigest::from_digest(custody.text_validation_digest),
        image::CompilerFunctionValidationDigest::from_digest(custody.function_validation_digest),
        custody.function_validation_report_fingerprint,
        executable_bytes.len(),
        executable_container_digest(executable_bytes),
    );
    if evidence.as_bytes() != &custody.evidence_digest {
        return reject(
            "native custody evidence",
            "recomputed publication evidence does not match the claimed digest",
        );
    }
    if let Err(rejection) = verify_terminal_artifact_proof(&psi_artifact, &policy.admission_profile)
    {
        return PccVerificationOutcome::Reject(rejection);
    }
    PccVerificationOutcome::Complete(PccVerifiedProduct {
        product: sidecar.product(),
        artifact_commitment: *sidecar.artifact_commitment(),
        policy_package_identity: policy.policy_package_identity.clone(),
        configuration_identity: policy.configuration_identity.clone(),
        accepted_guarantees: sidecar
            .guarantees()
            .iter()
            .map(|guarantee| guarantee.identity.clone())
            .collect(),
    })
}

fn reject(subject: impl Into<String>, reason: impl Into<String>) -> PccVerificationOutcome {
    PccVerificationOutcome::Reject(PccRejection::new(subject, reason))
}

struct CustodyCursor<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> CustodyCursor<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, len: usize) -> Result<&'bytes [u8], &'static str> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or("native custody evidence is truncated")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("native custody evidence is truncated")?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        self.take(N)?
            .try_into()
            .map_err(|_| "native custody evidence is truncated")
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_custody() -> NativePccCustodyEvidence {
        NativePccCustodyEvidence {
            psi_artifact_bytes: vec![1, 2, 3],
            native_artifact_identity: [4; 32],
            target: target::NativeTarget {
                architecture: target::Architecture::X86_64,
                object_format: target::ObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            final_image_symbol_digest: [5; 32],
            boundary_contract_report_fingerprint: Some(9),
            text_validation_digest: [6; 32],
            function_validation_digest: [7; 32],
            function_validation_report_fingerprint: 10,
            inventory_digest: [8; 32],
            inventory_report_fingerprint: 11,
            callback_placement_identity_report_fingerprint: 12,
            certificate_digest: [13; 32],
            evidence_digest: [14; 32],
        }
    }

    #[test]
    fn custody_round_trips() {
        let custody = sample_custody();
        assert_eq!(
            NativePccCustodyEvidence::from_bytes(&custody.to_bytes()).expect("decode"),
            custody
        );
    }

    #[test]
    fn custody_rejects_truncation_and_trailing() {
        let bytes = sample_custody().to_bytes();
        assert!(NativePccCustodyEvidence::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(NativePccCustodyEvidence::from_bytes(&longer).is_err());
    }
}
