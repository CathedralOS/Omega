use crate::{
    OfflinePolicyAlgorithmIdentity, OfflinePolicyCorpusIdentity, OfflinePolicyModelIdentity,
    OfflinePolicyReferenceError, OfflinePolicyReportIdentity, OfflinePolicySplitIdentity,
};

use super::{
    identity::OfflinePolicyRegressionManifestIdentity, model::OfflinePolicyRegressionManifest,
};
use crate::reference_policy::codec::{Cursor, decode_summary, encode_summary};

const MAGIC: &[u8; 8] = b"OMGORM\0\0";
const VERSION: u32 = 1;

pub(super) fn encode(manifest: &OfflinePolicyRegressionManifest) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&manifest.identity.bytes());
    encoded.extend_from_slice(&manifest.corpus.bytes());
    encoded.extend_from_slice(&manifest.model.bytes());
    encoded.extend_from_slice(&manifest.algorithm.bytes());
    encoded.extend_from_slice(&manifest.regression_split.bytes());
    encoded.extend_from_slice(&manifest.expected_report.bytes());
    encode_summary(&mut encoded, manifest.expected_summary);
    encoded
}

pub(super) fn decode(
    encoded: &[u8],
) -> Result<OfflinePolicyRegressionManifest, OfflinePolicyReferenceError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(OfflinePolicyReferenceError::WrongRegressionManifestMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != VERSION {
        return Err(OfflinePolicyReferenceError::UnsupportedRegressionManifestVersion(version));
    }
    let manifest = OfflinePolicyRegressionManifest {
        identity: OfflinePolicyRegressionManifestIdentity::from_bytes(cursor.array()?),
        corpus: OfflinePolicyCorpusIdentity::from_bytes(cursor.array()?),
        model: OfflinePolicyModelIdentity::from_bytes(cursor.array()?),
        algorithm: OfflinePolicyAlgorithmIdentity::from_bytes(cursor.array()?),
        regression_split: OfflinePolicySplitIdentity::from_bytes(cursor.array()?),
        expected_report: OfflinePolicyReportIdentity::from_bytes(cursor.array()?),
        expected_summary: decode_summary(&mut cursor)?,
    };
    if cursor.remaining() != 0 {
        return Err(OfflinePolicyReferenceError::TrailingBytes);
    }
    Ok(manifest)
}
