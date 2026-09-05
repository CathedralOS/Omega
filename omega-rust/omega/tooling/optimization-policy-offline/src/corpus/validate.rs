use std::collections::{BTreeMap, BTreeSet};

use optimization_core::OptimizationUnitIdentity;
use optimization_core::{ExternalDecisionLog, external_psi_decision_schema_v2_identity};

use super::identity::{corpus_identity, decision_surface_identity};
use super::model::{
    CorpusCandidate, OfflinePolicyCorpusError, OfflinePolicyCorpusReceipt,
    OfflinePolicyDecisionExample, OfflinePolicySplit, ValidatedOfflinePolicyCorpus,
};
use super::split::split_for_source;

pub(super) fn validate(
    candidate: CorpusCandidate,
) -> Result<ValidatedOfflinePolicyCorpus, OfflinePolicyCorpusError> {
    if candidate.logs.is_empty() {
        return Err(OfflinePolicyCorpusError::EmptyCorpus);
    }
    let mut prior_log = None;
    let mut sources = BTreeMap::<OptimizationUnitIdentity, OfflinePolicySplit>::new();
    let mut surfaces = BTreeSet::new();
    let mut examples = Vec::new();
    let mut split_counts = [0_u32; 3];

    for record in &candidate.logs {
        let log = ExternalDecisionLog::decode(&record.encoded)?;
        if log.encode() != record.encoded {
            return Err(OfflinePolicyCorpusError::NonCanonicalExternalLog);
        }
        if log.context().schema() != external_psi_decision_schema_v2_identity() {
            return Err(OfflinePolicyCorpusError::WrongExternalSchema);
        }
        if log.points().is_empty() {
            return Err(OfflinePolicyCorpusError::EmptyDecisionLog);
        }
        let identity = log.identity().bytes();
        if prior_log.is_some_and(|prior| prior >= identity) {
            return Err(if prior_log == Some(identity) {
                OfflinePolicyCorpusError::DuplicateLog
            } else {
                OfflinePolicyCorpusError::NonCanonicalLogs
            });
        }
        prior_log = Some(identity);

        let source = log.context().source();
        if sources
            .get(&source)
            .is_some_and(|prior| *prior != record.split)
        {
            return Err(OfflinePolicyCorpusError::SourceSplitLeakage);
        }
        let expected_split = split_for_source(source);
        if record.split != expected_split {
            return Err(OfflinePolicyCorpusError::SourceSplitMismatch);
        }
        sources.insert(source, record.split);

        for (ordinal, point) in log.points().iter().enumerate() {
            let surface = decision_surface_identity(log.context(), point);
            if !surfaces.insert(surface) {
                return Err(OfflinePolicyCorpusError::DuplicateDecisionSurface);
            }
            let point_ordinal =
                u32::try_from(ordinal).map_err(|_| OfflinePolicyCorpusError::CountOverflow)?;
            let slot = (record.split.tag() - 1) as usize;
            split_counts[slot] = split_counts[slot]
                .checked_add(1)
                .ok_or(OfflinePolicyCorpusError::CountOverflow)?;
            examples.push(OfflinePolicyDecisionExample {
                surface,
                log: log.identity(),
                point_ordinal,
                source,
                split: record.split,
                context: log.context(),
                point: point.clone(),
            });
        }
    }

    let identity = corpus_identity(&candidate.logs);
    if identity != candidate.claimed_identity {
        return Err(OfflinePolicyCorpusError::CorpusIdentityMismatch);
    }
    let receipt = OfflinePolicyCorpusReceipt {
        identity,
        schema: external_psi_decision_schema_v2_identity(),
        log_count: u32::try_from(candidate.logs.len())
            .map_err(|_| OfflinePolicyCorpusError::CountOverflow)?,
        source_count: u32::try_from(sources.len())
            .map_err(|_| OfflinePolicyCorpusError::CountOverflow)?,
        decision_count: u32::try_from(examples.len())
            .map_err(|_| OfflinePolicyCorpusError::CountOverflow)?,
        split_counts,
    };
    Ok(ValidatedOfflinePolicyCorpus {
        candidate,
        examples,
        receipt,
    })
}
