//! Source identity changes at established root or requester-local bindings.

use super::{PackagePolicyChangeError as Error, PackagePolicyChangeFingerprint, limits::Budget};
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::graph::{CanonicalDependencySourceSelection, CanonicalSourceClosureSubject};

/// The existing binding whose selected package changed. Neither package names
/// nor authored row positions pair otherwise unrelated graph changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyReplacementSite {
    Root,
    Dependency {
        requester: PackageKey,
        alias: AliasName,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicySourceReplacement {
    site: PackagePolicyReplacementSite,
    baseline: PackageKey,
    candidate: PackageKey,
    fingerprint: PackagePolicyChangeFingerprint,
}

impl PackagePolicySourceReplacement {
    pub const fn site(&self) -> &PackagePolicyReplacementSite {
        &self.site
    }

    pub const fn baseline(&self) -> &PackageKey {
        &self.baseline
    }

    pub const fn candidate(&self) -> &PackageKey {
        &self.candidate
    }

    pub const fn fingerprint(&self) -> PackagePolicyChangeFingerprint {
        self.fingerprint
    }
}

/// Root first, then shared requester/alias bindings in canonical order.
/// Different aliases remain separate additions/removals: a command changing
/// both identity and alias must retain its explicit replacement intent rather
/// than asking graph comparison to guess a pairing.
pub(super) fn compare(
    baseline: Option<&CanonicalSourceClosureSubject>,
    candidate: &CanonicalSourceClosureSubject,
    comparison: PackagePolicyChangeFingerprint,
    budget: &mut Budget,
) -> Result<Vec<PackagePolicySourceReplacement>, Error> {
    let Some(baseline) = baseline else {
        return Ok(Vec::new());
    };
    let old_root = baseline.root().selected().key();
    let new_root = candidate.root().selected().key();
    let old = ordered_bindings(baseline, budget)?;
    let new = ordered_bindings(candidate, budget)?;
    let mut count = usize::from(old_root != new_root);
    visit_replacements(&old, &new, |_, _| {
        count = count.checked_add(1).ok_or(Error::AllocationFailed)?;
        Ok(())
    })?;
    budget.source_replacements(count)?;
    budget.slots::<PackagePolicySourceReplacement>(count)?;
    let mut replacements = Vec::new();
    replacements
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    if old_root != new_root {
        push(
            &mut replacements,
            PackagePolicyReplacementSite::Root,
            old_root,
            new_root,
            comparison,
            budget,
        )?;
    }

    visit_replacements(&old, &new, |previous, current| {
        budget.key(previous.requester())?;
        budget.context(previous.alias().as_str().len())?;
        push(
            &mut replacements,
            PackagePolicyReplacementSite::Dependency {
                requester: previous.requester().clone(),
                alias: previous.alias().clone(),
            },
            previous.selected().key(),
            current.selected().key(),
            comparison,
            budget,
        )
    })?;
    Ok(replacements)
}

fn visit_replacements(
    old: &[&CanonicalDependencySourceSelection],
    new: &[&CanonicalDependencySourceSelection],
    mut visit: impl FnMut(
        &CanonicalDependencySourceSelection,
        &CanonicalDependencySourceSelection,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    let (mut old_index, mut new_index) = (0, 0);
    while let (Some(previous), Some(current)) = (old.get(old_index), new.get(new_index)) {
        match binding(previous).cmp(&binding(current)) {
            std::cmp::Ordering::Less => old_index += 1,
            std::cmp::Ordering::Greater => new_index += 1,
            std::cmp::Ordering::Equal => {
                if previous.selected().key() != current.selected().key() {
                    visit(previous, current)?;
                }
                old_index += 1;
                new_index += 1;
            }
        }
    }
    Ok(())
}

fn binding(selection: &CanonicalDependencySourceSelection) -> (&PackageKey, &str) {
    (selection.requester(), selection.alias().as_str())
}

fn ordered_bindings<'source>(
    source: &'source CanonicalSourceClosureSubject,
    budget: &mut Budget,
) -> Result<Vec<&'source CanonicalDependencySourceSelection>, Error> {
    let selections = source.dependency_requests();
    budget.slots::<&CanonicalDependencySourceSelection>(selections.len())?;
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(selections.len())
        .map_err(|_| Error::AllocationFailed)?;
    ordered.extend(selections);
    ordered.sort_unstable_by(|left, right| binding(left).cmp(&binding(right)));
    Ok(ordered)
}

fn push(
    replacements: &mut Vec<PackagePolicySourceReplacement>,
    site: PackagePolicyReplacementSite,
    baseline: &PackageKey,
    candidate: &PackageKey,
    comparison: PackagePolicyChangeFingerprint,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.key(baseline)?;
    budget.key(candidate)?;
    let fingerprint =
        super::fingerprints::source_replacement(comparison, &site, baseline, candidate);
    replacements.push(PackagePolicySourceReplacement {
        site,
        baseline: baseline.clone(),
        candidate: candidate.clone(),
        fingerprint,
    });
    Ok(())
}
