use std::collections::BTreeSet;

use optimization_core::{ExternalDecisionLog, external_psi_decision_schema_v2_identity};

use super::identity::corpus_identity;
use super::model::{CapturedLog, CorpusCandidate, OfflinePolicyCorpusError};
use super::split::split_for_source;

pub(super) fn capture<I, B>(encoded_logs: I) -> Result<CorpusCandidate, OfflinePolicyCorpusError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut logs = Vec::new();
    for encoded in encoded_logs {
        let authored = encoded.as_ref();
        let log = ExternalDecisionLog::decode(authored)?;
        if log.encode() != authored {
            return Err(OfflinePolicyCorpusError::NonCanonicalExternalLog);
        }
        if log.context().schema() != external_psi_decision_schema_v2_identity() {
            return Err(OfflinePolicyCorpusError::WrongExternalSchema);
        }
        if log.points().is_empty() {
            return Err(OfflinePolicyCorpusError::EmptyDecisionLog);
        }
        logs.push(CapturedLog {
            split: split_for_source(log.context().source()),
            encoded: authored.to_vec(),
        });
    }
    if logs.is_empty() {
        return Err(OfflinePolicyCorpusError::EmptyCorpus);
    }
    logs.sort_by_key(log_identity_bytes);
    let mut identities = BTreeSet::new();
    for log in &logs {
        if !identities.insert(log_identity_bytes(log)) {
            return Err(OfflinePolicyCorpusError::DuplicateLog);
        }
    }
    Ok(CorpusCandidate {
        claimed_identity: corpus_identity(&logs),
        logs,
    })
}

fn log_identity_bytes(log: &CapturedLog) -> [u8; 32] {
    ExternalDecisionLog::decode(&log.encoded)
        .expect("capture retains already decoded canonical logs")
        .identity()
        .bytes()
}
