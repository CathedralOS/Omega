use psi_terminal::TerminalModule;
use psi_terminal_verifier::ProofBundle;
use sha2::{Digest, Sha256};

use crate::{
    CodecError, ProofBundleFingerprint, ProofCodecError, TerminalPsiIdentity,
    proof_bundle_fingerprint, terminal_psi_identity,
};

const MANIFEST_FORMAT_MARKER: u16 = 1;
const INSTALLATION_DOMAIN: &[u8] = b"psi-terminal-installation-section\0";
const DEBUG_DOMAIN: &[u8] = b"psi-terminal-debug-section\0";
const ARTIFACT_DOMAIN: &[u8] = b"psi-terminal-artifact-manifest\0";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionFingerprint([u8; 32]);

impl SectionFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for SectionFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for SectionFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalArtifactIdentity([u8; 32]);

impl TerminalArtifactIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for TerminalArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for TerminalArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex(formatter, &self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalArtifactManifest {
    semantic: TerminalPsiIdentity,
    proof: ProofBundleFingerprint,
    installation: Option<SectionFingerprint>,
    debug: Option<SectionFingerprint>,
    identity: TerminalArtifactIdentity,
}

impl TerminalArtifactManifest {
    pub const fn semantic(self) -> TerminalPsiIdentity {
        self.semantic
    }

    pub const fn proof(self) -> ProofBundleFingerprint {
        self.proof
    }

    pub const fn installation(self) -> Option<SectionFingerprint> {
        self.installation
    }

    pub const fn debug(self) -> Option<SectionFingerprint> {
        self.debug
    }

    pub const fn identity(self) -> TerminalArtifactIdentity {
        self.identity
    }
}

pub fn build_artifact_manifest(
    semantic_module: &TerminalModule,
    proof_bundle: &ProofBundle,
    installation_record: Option<&[u8]>,
    debug_maps: Option<&[u8]>,
) -> Result<TerminalArtifactManifest, ArtifactManifestError> {
    let semantic =
        terminal_psi_identity(semantic_module).map_err(ArtifactManifestError::Semantic)?;
    let proof = proof_bundle_fingerprint(proof_bundle).map_err(ArtifactManifestError::Proof)?;
    let installation = installation_record.map(|bytes| hash_section(INSTALLATION_DOMAIN, bytes));
    let debug = debug_maps.map(|bytes| hash_section(DEBUG_DOMAIN, bytes));
    let identity = artifact_identity(semantic, proof, installation, debug);
    Ok(TerminalArtifactManifest {
        semantic,
        proof,
        installation,
        debug,
        identity,
    })
}

pub fn validate_artifact_manifest(
    semantic_module: &TerminalModule,
    proof_bundle: &ProofBundle,
    installation_record: Option<&[u8]>,
    debug_maps: Option<&[u8]>,
    manifest: TerminalArtifactManifest,
) -> Result<(), ArtifactManifestError> {
    let expected = build_artifact_manifest(
        semantic_module,
        proof_bundle,
        installation_record,
        debug_maps,
    )?;
    if expected != manifest {
        return Err(ArtifactManifestError::ManifestMismatch);
    }
    Ok(())
}

fn hash_section(domain: &[u8], bytes: &[u8]) -> SectionFingerprint {
    SectionFingerprint(hash(domain, bytes))
}

fn artifact_identity(
    semantic: TerminalPsiIdentity,
    proof: ProofBundleFingerprint,
    installation: Option<SectionFingerprint>,
    debug: Option<SectionFingerprint>,
) -> TerminalArtifactIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MANIFEST_FORMAT_MARKER.to_le_bytes());
    bytes.extend_from_slice(&semantic.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(semantic.program_fingerprint.as_bytes());
    bytes.extend_from_slice(proof.as_bytes());
    encode_optional_fingerprint(&mut bytes, installation);
    encode_optional_fingerprint(&mut bytes, debug);
    TerminalArtifactIdentity(hash(ARTIFACT_DOMAIN, &bytes))
}

fn encode_optional_fingerprint(bytes: &mut Vec<u8>, fingerprint: Option<SectionFingerprint>) {
    match fingerprint {
        None => bytes.push(0),
        Some(fingerprint) => {
            bytes.push(1);
            bytes.extend_from_slice(fingerprint.as_bytes());
        }
    }
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    let byte_len = u64::try_from(bytes.len()).expect("artifact section fits the digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn write_hex(formatter: &mut std::fmt::Formatter<'_>, bytes: &[u8; 32]) -> std::fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactManifestError {
    Semantic(CodecError),
    Proof(ProofCodecError),
    ManifestMismatch,
}

impl std::fmt::Display for ArtifactManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ArtifactManifestError {}
