use super::{Cursor, decode_summary, encode_summary};
use crate::ValidatedOfflinePolicyCorpus;

use super::super::{
    identity::{
        OfflinePolicyAlgorithmIdentity, OfflinePolicyModelIdentity, OfflinePolicySplitIdentity,
    },
    model::{CostThresholdV1Model, OfflinePolicyReferenceError},
    training,
};

const MAGIC: &[u8; 8] = b"OMGOPM\0\0";
const VERSION: u32 = 1;

pub(crate) fn encode(model: &CostThresholdV1Model) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&model.identity.bytes());
    encoded.extend_from_slice(&model.corpus.bytes());
    encoded.extend_from_slice(&model.algorithm.bytes());
    encoded.extend_from_slice(&model.training_split.bytes());
    encoded.extend_from_slice(&model.threshold.to_le_bytes());
    encode_summary(&mut encoded, model.training);
    encoded
}

pub(crate) fn decode(
    encoded: &[u8],
    corpus: &ValidatedOfflinePolicyCorpus,
) -> Result<CostThresholdV1Model, OfflinePolicyReferenceError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(OfflinePolicyReferenceError::WrongModelMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != VERSION {
        return Err(OfflinePolicyReferenceError::UnsupportedModelVersion(
            version,
        ));
    }
    let model = CostThresholdV1Model {
        identity: OfflinePolicyModelIdentity::from_bytes(cursor.array()?),
        corpus: crate::OfflinePolicyCorpusIdentity::from_bytes(cursor.array()?),
        algorithm: OfflinePolicyAlgorithmIdentity::from_bytes(cursor.array()?),
        training_split: OfflinePolicySplitIdentity::from_bytes(cursor.array()?),
        threshold: i128::from_le_bytes(cursor.array()?),
        training: decode_summary(&mut cursor)?,
    };
    if cursor.remaining() != 0 {
        return Err(OfflinePolicyReferenceError::TrailingBytes);
    }
    training::validate(&model, corpus)?;
    Ok(model)
}
