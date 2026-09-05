use super::*;
use std::cmp::Ordering;

pub(super) fn compare(
    left: &HistoricalPackagePolicyDecision,
    right: &HistoricalPackagePolicyDecision,
) -> Ordering {
    subject_order(&left.subject, &right.subject).then(left.conflict.cmp(&right.conflict))
}

fn subject_order(left: &Subject, right: &Subject) -> Ordering {
    fn tag(value: &Subject) -> u8 {
        match value {
            Subject::CandidatePackage { .. } => 0,
            Subject::RemovedPackage { .. } => 1,
            Subject::RootRole { .. } => 2,
        }
    }
    match (left, right) {
        (
            Subject::CandidatePackage {
                package_index: left,
            },
            Subject::CandidatePackage {
                package_index: right,
            },
        ) => left.cmp(right),
        (Subject::RemovedPackage { key: left }, Subject::RemovedPackage { key: right }) => {
            left.cmp(right)
        }
        (
            Subject::RootRole {
                package_index: left,
                ..
            },
            Subject::RootRole {
                package_index: right,
                ..
            },
        ) => left.cmp(right),
        _ => tag(left).cmp(&tag(right)),
    }
}

/// Fingerprint scratch is charged by the caller before invoking this owner.
pub(super) fn validate(
    history: &HistoricalPackagePolicyDecisions,
    source: &Source,
) -> Result<(), Error> {
    if history.source_subject() != source.fingerprint() {
        return Err(Error::SourceSubjectMismatch);
    }
    if history.comparison.is_none() {
        return super::super::text::validate_decisions(&history.decisions, source.packages().len());
    }
    let mut root_role_count = 0usize;
    for decision in &history.decisions {
        match &decision.subject {
            Subject::CandidatePackage { package_index }
                if *package_index < source.packages().len() => {}
            Subject::RemovedPackage { key }
                if source
                    .packages()
                    .binary_search_by(|source| source.key().cmp(key))
                    .is_err() => {}
            Subject::RootRole {
                package_index,
                baseline_role,
                candidate_role,
                broken_contract,
            } => {
                root_role_count += 1;
                if root_role_count > 1
                    || source
                        .packages()
                        .get(*package_index)
                        .map(|source| source.key())
                        != Some(source.root().selected().key())
                    || *candidate_role != source.root_role()
                    || !matches!(
                        (*baseline_role, *candidate_role, *broken_contract),
                        (
                            BuildDeclarationKind::Package,
                            BuildDeclarationKind::Application,
                            ReviewOnlyRootRoleContract::DependencyCompatibility
                        ) | (
                            BuildDeclarationKind::Application,
                            BuildDeclarationKind::Package,
                            ReviewOnlyRootRoleContract::ApplicationActivation
                        )
                    )
                {
                    return Err(Error::InvalidSubject);
                }
            }
            _ => return Err(Error::InvalidSubject),
        }
    }
    if history
        .decisions
        .windows(2)
        .any(|pair| !compare(&pair[0], &pair[1]).is_lt())
    {
        return Err(Error::NonCanonicalDecisions);
    }
    let mut fingerprints = Vec::new();
    fingerprints
        .try_reserve_exact(history.decisions.len())
        .map_err(|_| Error::AllocationFailed)?;
    fingerprints.extend(history.decisions.iter().map(|row| row.conflict));
    fingerprints.sort_unstable();
    if fingerprints.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::NonCanonicalDecisions);
    }
    Ok(())
}
