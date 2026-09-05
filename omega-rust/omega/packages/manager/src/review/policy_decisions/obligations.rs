use super::{
    PackagePolicyDecision, PackagePolicyDecisionError as Error, PackagePolicyDecisionLimits,
    PackagePolicyDecisionObligation, PackagePolicyDecisionSubject,
    PackagePolicyObligationFingerprint, limits::Budget,
};
use crate::declarations::BuildDeclarationKind;
use crate::review::{
    PackagePolicyChangeSet, PackagePolicyPackageChange, PackagePolicyRowChange,
    ReviewOnlyRootPolicyDisposition, ReviewOnlyRootRoleContract,
};
use psi_core::PackageKeyIdentity;
use sha2::{Digest, Sha256};

impl PackagePolicyChangeSet {
    /// Enumerate precisely the current blocking rows and directional role loss.
    /// Audit recommendations and root-key changes do not mint obligations.
    pub fn decision_obligations(
        &self,
        limits: PackagePolicyDecisionLimits,
    ) -> Result<Vec<PackagePolicyDecisionObligation>, Error> {
        collect(self, &mut Budget::new(limits))
    }

    /// Constant-time construction from this exact comparison's privately issued
    /// obligation. The complete resolver independently rejoins every decision.
    pub fn policy_decision(
        &self,
        obligation: &PackagePolicyDecisionObligation,
        disposition: ReviewOnlyRootPolicyDisposition,
    ) -> Result<PackagePolicyDecision, Error> {
        if obligation.change_set != self.fingerprint() {
            return Err(Error::WrongChangeSet);
        }
        Ok(PackagePolicyDecision {
            obligation: *obligation,
            disposition,
        })
    }

    /// Select a row through its current package and stable semantic coordinate.
    pub fn row_policy_decision(
        &self,
        package: &PackagePolicyPackageChange,
        row: &PackagePolicyRowChange,
        disposition: ReviewOnlyRootPolicyDisposition,
    ) -> Result<PackagePolicyDecision, Error> {
        let index = self
            .packages()
            .binary_search_by(|current| current.key().cmp(package.key()))
            .map_err(|_| Error::ForeignPackage)?;
        let owned = &self.packages()[index];
        if owned.fingerprint() != package.fingerprint() {
            return Err(Error::WrongChangeSet);
        }
        let index = owned
            .rows()
            .binary_search_by(|current| {
                (current.kind(), current.key_bytes()).cmp(&(row.kind(), row.key_bytes()))
            })
            .map_err(|_| Error::StaleOrForeignObligation)?;
        let owned_row = &owned.rows()[index];
        if owned_row.fingerprint() != row.fingerprint() {
            return Err(Error::StaleOrForeignObligation);
        }
        if !owned_row.requires_decision() {
            return Err(Error::NonBlockingRow);
        }
        let obligation = issue(
            self,
            owned.key().identity(),
            PackagePolicyDecisionSubject::Row {
                kind: owned_row.kind(),
                fingerprint: owned_row.fingerprint(),
            },
        );
        self.policy_decision(&obligation, disposition)
    }
}

pub(super) fn collect(
    changes: &PackagePolicyChangeSet,
    budget: &mut Budget,
) -> Result<Vec<PackagePolicyDecisionObligation>, Error> {
    let mut scanned = changes.packages().len();
    let mut count = usize::from(changes.root_role_change().is_some());
    scanned = scanned.checked_add(count).ok_or(Error::LengthOverflow)?;
    for package in changes.packages() {
        scanned = scanned
            .checked_add(package.rows().len())
            .ok_or(Error::LengthOverflow)?;
        if scanned > budget.limits.maximum_changes {
            return Err(Error::ChangeLimitExceeded);
        }
        count = count
            .checked_add(
                package
                    .rows()
                    .iter()
                    .filter(|row| row.requires_decision())
                    .count(),
            )
            .ok_or(Error::LengthOverflow)?;
    }
    if scanned > budget.limits.maximum_changes {
        return Err(Error::ChangeLimitExceeded);
    }
    budget.decisions(count)?;
    let mut result = budget.vector(count)?;
    for package in changes.packages() {
        let package_identity = package.key().identity();
        for row in package.rows().iter().filter(|row| row.requires_decision()) {
            result.push(issue(
                changes,
                package_identity,
                PackagePolicyDecisionSubject::Row {
                    kind: row.kind(),
                    fingerprint: row.fingerprint(),
                },
            ));
        }
    }
    if let Some(role) = changes.root_role_change() {
        result.push(issue(
            changes,
            role.root().identity(),
            PackagePolicyDecisionSubject::RootRole {
                baseline_role: role.baseline_role(),
                candidate_role: role.candidate_role(),
                broken_contract: role.broken_contract(),
            },
        ));
    }
    result.sort_unstable_by_key(|obligation| obligation.fingerprint);
    if result
        .windows(2)
        .any(|pair| pair[0].fingerprint == pair[1].fingerprint)
    {
        return Err(Error::DuplicateObligation);
    }
    Ok(result)
}

fn issue(
    changes: &PackagePolicyChangeSet,
    package: PackageKeyIdentity,
    subject: PackagePolicyDecisionSubject,
) -> PackagePolicyDecisionObligation {
    let mut hash = Sha256::new();
    match subject {
        PackagePolicyDecisionSubject::Row { .. } => {
            hash.update(b"OMEGA-NORMALIZED-POLICY-ROW-OBLIGATION\0")
        }
        PackagePolicyDecisionSubject::RootRole { .. } => {
            hash.update(b"OMEGA-NORMALIZED-POLICY-ROOT-ROLE-OBLIGATION\0")
        }
    }
    hash.update(1_u16.to_le_bytes());
    hash.update(changes.fingerprint().digest());
    hash.update(package.digest());
    match subject {
        PackagePolicyDecisionSubject::Row { kind, fingerprint } => {
            hash.update([kind.canonical_tag()]);
            hash.update(fingerprint.digest());
        }
        PackagePolicyDecisionSubject::RootRole {
            baseline_role,
            candidate_role,
            broken_contract,
        } => {
            hash.update([
                role_tag(baseline_role),
                role_tag(candidate_role),
                match broken_contract {
                    ReviewOnlyRootRoleContract::DependencyCompatibility => 1,
                    ReviewOnlyRootRoleContract::ApplicationActivation => 2,
                },
            ]);
        }
    }
    PackagePolicyDecisionObligation {
        change_set: changes.fingerprint(),
        package,
        subject,
        fingerprint: PackagePolicyObligationFingerprint(hash.finalize().into()),
    }
}
fn role_tag(role: BuildDeclarationKind) -> u8 {
    match role {
        BuildDeclarationKind::Package => 1,
        BuildDeclarationKind::Application => 2,
        BuildDeclarationKind::Workspace => 3,
    }
}
