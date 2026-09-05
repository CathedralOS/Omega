use super::{
    PackagePolicyDecision, PackagePolicyDecisionError as Error, PackagePolicyDecisionLimits,
    PackagePolicyDecisionObligation, PackagePolicyDecisionResolution,
    PackagePolicyDecisionResolutionFingerprint, limits::Budget, obligations,
};
use crate::review::{PackagePolicyChangeSet, ReviewOnlyRootPolicyDisposition};
use sha2::{Digest, Sha256};

/// Require exactly one explicit choice for every current blocker. Rejections
/// produce a complete negative resolution; no blockers plus no choices produce
/// an explicit empty resolution. Neither result discharges compiler obligations.
pub fn resolve_package_policy_decisions(
    changes: &PackagePolicyChangeSet,
    decisions: &[PackagePolicyDecision],
    limits: PackagePolicyDecisionLimits,
) -> Result<PackagePolicyDecisionResolution, Error> {
    let mut budget = Budget::new(limits);
    budget.decisions(decisions.len())?;
    let known = obligations::collect(changes, &mut budget)?;
    exact_count(decisions.len(), known.len())?;
    let mut canonical = budget.vector(decisions.len())?;
    canonical.extend_from_slice(decisions);
    finish(changes, &known, canonical)
}

pub(super) fn exact_count(actual: usize, expected: usize) -> Result<(), Error> {
    match actual.cmp(&expected) {
        std::cmp::Ordering::Less => Err(Error::MissingDecision),
        std::cmp::Ordering::Greater => Err(Error::StaleOrForeignObligation),
        std::cmp::Ordering::Equal => Ok(()),
    }
}

pub(super) fn finish(
    changes: &PackagePolicyChangeSet,
    known: &[PackagePolicyDecisionObligation],
    mut decisions: Vec<PackagePolicyDecision>,
) -> Result<PackagePolicyDecisionResolution, Error> {
    for decision in &decisions {
        if decision.change_set() != changes.fingerprint() {
            return Err(Error::WrongChangeSet);
        }
    }
    decisions.sort_unstable_by_key(|decision| decision.obligation.fingerprint);
    if decisions
        .windows(2)
        .any(|pair| pair[0].obligation.fingerprint == pair[1].obligation.fingerprint)
    {
        return Err(Error::DuplicateDecision);
    }
    // Sorted merge is linear after sorting. No repeated blocker enumeration or
    // source-policy projection occurs while constructing N decisions.
    let mut index = 0;
    for decision in &decisions {
        let Some(obligation) = known.get(index) else {
            return Err(Error::StaleOrForeignObligation);
        };
        match decision.obligation.fingerprint.cmp(&obligation.fingerprint) {
            std::cmp::Ordering::Less => return Err(Error::StaleOrForeignObligation),
            std::cmp::Ordering::Greater => return Err(Error::MissingDecision),
            std::cmp::Ordering::Equal => {}
        }
        if decision.package() != obligation.package {
            return Err(Error::ForeignPackage);
        }
        if decision.obligation != *obligation {
            return Err(Error::StaleOrForeignObligation);
        }
        index += 1;
    }
    if index != known.len() {
        return Err(Error::MissingDecision);
    }
    let all_required_changes_accepted = decisions.iter().all(|decision| {
        decision.disposition == ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
    });
    let mut hash = Sha256::new();
    hash.update(b"OMEGA-NORMALIZED-POLICY-DECISION-RESOLUTION\0");
    hash.update(1_u16.to_le_bytes());
    hash.update(changes.fingerprint().digest());
    hash.update((decisions.len() as u64).to_le_bytes());
    for decision in &decisions {
        hash.update(decision.package().digest());
        hash.update(decision.obligation.fingerprint.digest());
        hash.update([match decision.disposition {
            ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => 1,
            ReviewOnlyRootPolicyDisposition::RejectCandidateChange => 2,
        }]);
    }
    Ok(PackagePolicyDecisionResolution {
        change_set: changes.fingerprint(),
        decisions,
        fingerprint: PackagePolicyDecisionResolutionFingerprint(hash.finalize().into()),
        all_required_changes_accepted,
    })
}
