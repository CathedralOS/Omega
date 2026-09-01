//! Optimizer module role: stage group. Strict offline corpus wire codec.

mod cursor;

use super::model::{CapturedLog, CorpusCandidate, OfflinePolicyCorpusError, OfflinePolicySplit};
use cursor::Cursor;

const MAGIC: &[u8; 8] = b"OMGOPC\0\0";
const VERSION: u32 = 1;
const HEADER_LENGTH: usize = 8 + 4 + 32 + 4;

pub(super) fn encode(candidate: &CorpusCandidate) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&candidate.claimed_identity.bytes());
    encoded.extend_from_slice(
        &u32::try_from(candidate.logs.len())
            .expect("validated corpus log count fits u32")
            .to_le_bytes(),
    );
    for record in &candidate.logs {
        encoded.push(record.split.tag());
        encoded.extend_from_slice(
            &u32::try_from(record.encoded.len())
                .expect("validated external log encoding fits u32")
                .to_le_bytes(),
        );
        encoded.extend_from_slice(&record.encoded);
    }
    encoded
}

pub(super) fn decode(encoded: &[u8]) -> Result<CorpusCandidate, OfflinePolicyCorpusError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(OfflinePolicyCorpusError::WrongMagic);
    }
    let version = u32::from_le_bytes(cursor.array()?);
    if version != VERSION {
        return Err(OfflinePolicyCorpusError::UnsupportedVersion(version));
    }
    let claimed_identity =
        super::identity::OfflinePolicyCorpusIdentity::from_bytes(cursor.array()?);
    let count = u32::from_le_bytes(cursor.array()?) as usize;
    if count > cursor.remaining() / 5 {
        return Err(OfflinePolicyCorpusError::Truncated);
    }
    let mut logs = Vec::with_capacity(count);
    for _ in 0..count {
        let split = OfflinePolicySplit::from_tag(cursor.byte()?)?;
        let length = u32::from_le_bytes(cursor.array()?) as usize;
        logs.push(CapturedLog {
            split,
            encoded: cursor.take(length)?.to_vec(),
        });
    }
    if cursor.remaining() != 0 {
        return Err(OfflinePolicyCorpusError::TrailingBytes);
    }
    debug_assert_eq!(HEADER_LENGTH, 48);
    Ok(CorpusCandidate {
        claimed_identity,
        logs,
    })
}
