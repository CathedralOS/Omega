//! Optimizer module role: executable entrance. Frames and dispatches the v10 function-relative realization manifest protocol.

mod cursor;
mod decoding;
mod encoding;
mod error;
mod post_allocation;
mod rendering;
mod target;

use super::model::FunctionRelativeOptimizationRealizationManifest;
use super::prelude::*;

use cursor::Cursor;
pub use error::FunctionRelativeOptimizationRealizationManifestDecodeError;

const MANIFEST_MAGIC: &[u8; 8] = b"OMGFRM\0\0";
const MANIFEST_VERSION: u32 = 10;

impl FunctionRelativeOptimizationRealizationManifest {
    pub fn recomputed_identity(&self) -> FunctionRelativeOptimizationRealizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical
            .extend_from_slice(b"omega.function-relative-optimization-realization-manifest.v10\0");
        canonical.extend_from_slice(&encoding::encode_manifest_content(self));
        FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(&canonical)
    }

    pub fn encode(&self) -> Vec<u8> {
        let content = encoding::encode_manifest_content(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MANIFEST_MAGIC);
        encoded.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        encoded.extend_from_slice(&self.identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MANIFEST_MAGIC.len())? != MANIFEST_MAGIC {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != MANIFEST_VERSION {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity =
            FunctionRelativeOptimizationRealizationManifestIdentity::from_bytes(cursor.array()?);
        let manifest = decoding::decode_manifest_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(FunctionRelativeOptimizationRealizationManifestDecodeError::TrailingBytes);
        }
        if manifest.x86_branch_relaxation.is_some()
            && manifest.post_allocation_machine_optimization.is_some()
        {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::ConflictingPhysicalTransformations,
            );
        }
        if manifest.identity != manifest.recomputed_identity() {
            return Err(
                FunctionRelativeOptimizationRealizationManifestDecodeError::IdentityMismatch,
            );
        }
        Ok(manifest)
    }

    pub fn render_text(&self) -> String {
        rendering::render_manifest(self)
    }
}
