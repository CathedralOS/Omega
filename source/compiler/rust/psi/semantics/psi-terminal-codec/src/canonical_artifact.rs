//! Canonical, source-free Terminal-Psi handoff artifact.
//!
//! This is the exact ownership seam between Psi semantic production and Omega
//! target realization. It contains only canonical semantic, proof, and
//! optional debug bytes plus their manifest identity. Target selection,
//! provider realization, deployment policy, output paths, and installation
//! authority are deliberately absent.

use psi_terminal::TerminalModule;
use psi_terminal_verifier::ProofBundle;

use crate::{
    ArtifactManifestError, CodecError, DebugMapError, ProofCodecError, TerminalArtifactManifest,
    TerminalDebugMap, build_artifact_manifest, decode_debug_map, decode_module,
    decode_proof_bundle, encode_debug_map, encode_module, encode_proof_bundle,
    validate_artifact_manifest,
};

/// One exact canonical Terminal-Psi semantic/proof artifact.
#[derive(Debug, PartialEq, Eq)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    semantic_bytes: Vec<u8>,
    proof_bytes: Vec<u8>,
    debug_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
}

impl CanonicalTerminalArtifact {
    /// Encode and independently replay one source-free Terminal artifact.
    pub fn from_parts(
        semantic_module: &TerminalModule,
        proof_bundle: &ProofBundle,
        debug_map: Option<&TerminalDebugMap>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let semantic_bytes =
            encode_module(semantic_module).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let proof_bytes =
            encode_proof_bundle(proof_bundle).map_err(CanonicalTerminalArtifactError::Proof)?;
        let debug_bytes = debug_map
            .map(|debug_map| {
                encode_debug_map(semantic_module, debug_map)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let manifest =
            build_artifact_manifest(semantic_module, proof_bundle, None, debug_bytes.as_deref())
                .map_err(CanonicalTerminalArtifactError::Manifest)?;
        let artifact = Self {
            semantic_bytes,
            proof_bytes,
            debug_bytes,
            manifest,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    /// Re-decode every canonical section and reconstruct the manifest.
    pub fn validate(&self) -> Result<(), CanonicalTerminalArtifactError> {
        let semantic_module = decode_module(&self.semantic_bytes)
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let proof_bundle = decode_proof_bundle(&self.proof_bytes)
            .map_err(CanonicalTerminalArtifactError::Proof)?;
        if let Some(debug_bytes) = self.debug_bytes.as_deref() {
            decode_debug_map(&semantic_module, debug_bytes)
                .map_err(CanonicalTerminalArtifactError::Debug)?;
        }
        validate_artifact_manifest(
            &semantic_module,
            &proof_bundle,
            None,
            self.debug_bytes.as_deref(),
            self.manifest,
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.semantic_bytes
    }

    pub fn proof_bytes(&self) -> &[u8] {
        &self.proof_bytes
    }

    pub fn debug_bytes(&self) -> Option<&[u8]> {
        self.debug_bytes.as_deref()
    }

    pub const fn manifest(&self) -> TerminalArtifactManifest {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactError {
    Semantic(CodecError),
    Proof(ProofCodecError),
    Debug(DebugMapError),
    Manifest(ArtifactManifestError),
}

impl std::fmt::Display for CanonicalTerminalArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CanonicalTerminalArtifactError {}
