use omega_optimization_core::PostAllocationOptimizationManifestIdentity;

use super::{PostAllocationOptimizationManifest, codec::encode_manifest_content};

impl PostAllocationOptimizationManifest {
    pub fn recomputed_identity(&self) -> PostAllocationOptimizationManifestIdentity {
        let mut canonical = Vec::new();
        canonical.extend_from_slice(b"omega.post-allocation-optimization-manifest.v6\0");
        canonical.extend_from_slice(&encode_manifest_content(self));
        PostAllocationOptimizationManifestIdentity::from_canonical_bytes(&canonical)
    }
}
