use sha2::{Digest, Sha256};

use super::model::OfflinePolicyRegressionManifest;
use crate::reference_policy::identity::encode_summary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OfflinePolicyRegressionManifestIdentity([u8; 32]);

impl OfflinePolicyRegressionManifestIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

pub(super) fn identity(
    manifest: &OfflinePolicyRegressionManifest,
) -> OfflinePolicyRegressionManifestIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.offline-policy.regression-manifest.sha256.v1\0");
    digest.update(manifest.corpus.bytes());
    digest.update(manifest.model.bytes());
    digest.update(manifest.algorithm.bytes());
    digest.update(manifest.regression_split.bytes());
    digest.update(manifest.expected_report.bytes());
    encode_summary(&mut digest, manifest.expected_summary);
    OfflinePolicyRegressionManifestIdentity(digest.finalize().into())
}
