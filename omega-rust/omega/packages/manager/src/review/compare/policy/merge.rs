use super::{
    PackagePolicyChangeError as Error, PackagePolicyChangeFingerprint, PackagePolicyChangeKind,
    PackagePolicyRowChange,
    limits::{Budget, row_bytes},
};
use omega_package_evidence::record::PackagePolicyRow;
use std::cmp::Ordering;

pub(super) fn rows(
    baseline: Vec<PackagePolicyRow>,
    candidate: Vec<PackagePolicyRow>,
    existing_package: bool,
    budget: &mut Budget,
) -> Result<Vec<PackagePolicyRowChange>, Error> {
    let mut count = 0usize;
    let mut bytes = 0usize;
    walk(&baseline, &candidate, |old, new| {
        if old == new {
            return Ok(());
        }
        count = count.checked_add(1).ok_or(Error::AllocationFailed)?;
        for row in [old, new].into_iter().flatten() {
            bytes = bytes
                .checked_add(row_bytes(row)?)
                .ok_or(Error::AllocationFailed)?;
        }
        Ok(())
    })?;
    budget.changed(count, bytes)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    let mut old = baseline.into_iter().peekable();
    let mut new = candidate.into_iter().peekable();
    while old.peek().is_some() || new.peek().is_some() {
        let order = compare(old.peek(), new.peek());
        let previous = if order.is_gt() { None } else { old.next() };
        let current = if order.is_lt() { None } else { new.next() };
        if previous == current {
            continue;
        }
        let change = match (&previous, &current) {
            (None, Some(_)) => PackagePolicyChangeKind::Added,
            (Some(_), None) => PackagePolicyChangeKind::Removed,
            (Some(_), Some(_)) => PackagePolicyChangeKind::Changed,
            (None, None) => unreachable!("row union contains one side"),
        };
        let requires_decision = if existing_package {
            previous
                .as_ref()
                .is_some_and(PackagePolicyRow::update_requires_decision)
                || current
                    .as_ref()
                    .is_some_and(PackagePolicyRow::update_requires_decision)
        } else {
            current
                .as_ref()
                .is_some_and(PackagePolicyRow::initial_requires_decision)
        };
        // Introduction is a change from absence. Representation choices need
        // that audit recommendation even without a previous package contract;
        // unchanged presence is a separate, narrower audit classification.
        let audit_recommended = previous
            .as_ref()
            .is_some_and(PackagePolicyRow::audit_recommended_on_change)
            || current
                .as_ref()
                .is_some_and(PackagePolicyRow::audit_recommended_on_change);
        result.push(PackagePolicyRowChange {
            baseline: previous,
            candidate: current,
            change,
            requires_decision,
            audit_recommended,
            fingerprint: PackagePolicyChangeFingerprint([0; 32]),
        });
    }
    Ok(result)
}

fn compare(old: Option<&PackagePolicyRow>, new: Option<&PackagePolicyRow>) -> Ordering {
    match (old, new) {
        (Some(old), Some(new)) => (old.kind(), old.key_bytes()).cmp(&(new.kind(), new.key_bytes())),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
fn walk(
    old: &[PackagePolicyRow],
    new: &[PackagePolicyRow],
    mut visit: impl FnMut(Option<&PackagePolicyRow>, Option<&PackagePolicyRow>) -> Result<(), Error>,
) -> Result<(), Error> {
    let (mut left, mut right) = (0, 0);
    while left < old.len() || right < new.len() {
        let order = compare(old.get(left), new.get(right));
        let previous = if order.is_gt() {
            None
        } else {
            let value = old.get(left);
            left += 1;
            value
        };
        let current = if order.is_lt() {
            None
        } else {
            let value = new.get(right);
            right += 1;
            value
        };
        visit(previous, current)?;
    }
    Ok(())
}
