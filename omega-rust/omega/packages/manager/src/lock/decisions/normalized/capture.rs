use super::*;
use crate::review::{
    PackagePolicyChangeSet, PackagePolicyDecisionSubject, PackagePolicyResolution,
};
use sha2::{Digest, Sha256};

impl HistoricalPackagePolicyDecisions {
    /// Record the exact completed normalized choices as inert V2 history.
    /// Rejections are retained; this operation never authorizes a candidate.
    pub fn capture_policy_changes(
        source: &Source,
        changes: &PackagePolicyChangeSet,
        resolution: &PackagePolicyResolution,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::capture_policy_changes_with_usage(source, changes, resolution, limits, usize::MAX)
            .map(|(history, _)| history)
    }

    pub fn capture_policy_changes_with_usage(
        source: &Source,
        changes: &PackagePolicyChangeSet,
        resolution: &PackagePolicyResolution,
        limits: Limits,
        maximum_owned_bytes: usize,
    ) -> Result<(Self, Usage), Error> {
        let limits = limits.bounded();
        if source.fingerprint() != changes.candidate_source_subject() {
            return Err(Error::SourceSubjectMismatch);
        }
        if resolution.comparison() != changes.fingerprint() {
            return Err(Error::ResolutionMismatch);
        }
        let count = resolution.decisions().len();
        if count > limits.maximum_decisions {
            return Err(Error::DecisionLimitExceeded);
        }
        let mut usage = Usage {
            owned_bytes: 0,
            decisions: count,
        };
        usage.charge(
            count
                .checked_mul(std::mem::size_of::<HistoricalPackagePolicyDecision>())
                .ok_or(Error::AllocationLimitExceeded)?,
            maximum_owned_bytes,
        )?;
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        // One temporary row index joins upstream decision subjects back to
        // their exact package without rebuilding compiler review or policy.
        type Index = ([u8; 32], usize);
        usage.charge(
            count
                .checked_mul(std::mem::size_of::<Index>())
                .ok_or(Error::AllocationLimitExceeded)?,
            maximum_owned_bytes,
        )?;
        let mut index = Vec::new();
        index
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        for (ordinal, package) in changes.packages().iter().enumerate() {
            for row in package.rows().iter().filter(|row| row.requires_decision()) {
                if index.len() == count {
                    return Err(Error::ResolutionMismatch);
                }
                index.push((row.fingerprint().digest(), ordinal));
            }
        }
        index.sort_unstable();
        if index.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::ResolutionMismatch);
        }
        let mut removed_text_bytes = 0usize;
        for decision in resolution.decisions() {
            let (ordinal, conflict) = match decision.subject {
                PackagePolicyDecisionSubject::Row(fingerprint) => {
                    let entry = index
                        .binary_search_by_key(&fingerprint, |entry| entry.0)
                        .map_err(|_| Error::ResolutionMismatch)?;
                    (index[entry].1, fingerprint)
                }
                PackagePolicyDecisionSubject::RootRole => {
                    let role = changes.root_role_change().ok_or(Error::InvalidSubject)?;
                    let ordinal = changes
                        .packages()
                        .binary_search_by(|package| package.key().cmp(role.root()))
                        .map_err(|_| Error::UnknownPackage)?;
                    let mut hash = Sha256::new();
                    hash.update(b"OMEGA-HISTORICAL-ROOT-ROLE-DECISION\0");
                    hash.update(1_u16.to_le_bytes());
                    hash.update(changes.fingerprint().digest());
                    (ordinal, hash.finalize().into())
                }
            };
            let package = &changes.packages()[ordinal];
            let candidate = source
                .packages()
                .binary_search_by(|entry| entry.key().cmp(package.key()));
            let subject = match decision.subject {
                PackagePolicyDecisionSubject::Row(_) => match package.candidate_resolution() {
                    Some(expected) => {
                        let package_index = candidate.map_err(|_| Error::UnknownPackage)?;
                        if source.packages()[package_index].resolution() != expected {
                            return Err(Error::ResolutionMismatch);
                        }
                        Subject::CandidatePackage { package_index }
                    }
                    None => {
                        if candidate.is_ok() {
                            return Err(Error::InvalidSubject);
                        }
                        let (fragment, owned) = write_package_key_text(
                            package.key(),
                            key_limits(limits.maximum_bytes),
                            maximum_owned_bytes - usage.owned_bytes,
                        )
                        .map_err(source_key_error)?;
                        usage.charge(owned, maximum_owned_bytes)?;
                        // One removed package can own many blocking rows. Bound
                        // repeated key expansion before retaining another copy,
                        // including for the default unbounded usage caller.
                        // The final writer separately checks framing overhead.
                        removed_text_bytes = removed_text_bytes
                            .checked_add(fragment.len())
                            .filter(|bytes| *bytes <= limits.maximum_bytes)
                            .ok_or(Error::ByteLimitExceeded)?;
                        let (key, owned) = recover_package_key_text(
                            &fragment,
                            key_limits(limits.maximum_bytes),
                            maximum_owned_bytes - usage.owned_bytes,
                        )
                        .map_err(source_key_error)?;
                        usage.charge(owned, maximum_owned_bytes)?;
                        Subject::RemovedPackage { key }
                    }
                },
                PackagePolicyDecisionSubject::RootRole => {
                    let change = changes.root_role_change().ok_or(Error::InvalidSubject)?;
                    Subject::RootRole {
                        package_index: candidate.map_err(|_| Error::UnknownPackage)?,
                        baseline_role: change.baseline_role(),
                        candidate_role: change.candidate_role(),
                        broken_contract: change.broken_contract(),
                    }
                }
            };
            decisions.push(HistoricalPackagePolicyDecision {
                subject,
                conflict,
                disposition: decision.disposition,
            });
        }
        decisions.sort_unstable_by(validation::compare);
        let history = Self {
            source_subject: source.fingerprint().clone(),
            comparison: Some(changes.fingerprint().digest()),
            decisions,
        };
        let (_, serialization) = history.canonical_text_with_usage(
            source,
            limits,
            maximum_owned_bytes - usage.owned_bytes,
        )?;
        usage.charge(serialization.owned_bytes(), maximum_owned_bytes)?;
        Ok((history, usage))
    }
}
