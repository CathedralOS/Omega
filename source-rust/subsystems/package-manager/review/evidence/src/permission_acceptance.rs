//! Compare a retained lock decision with a complete proposed policy. This comparison
//! describes agreement between records; runtime admission also needs a checked issuer.
use crate::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyBaseline, PackagePolicyRowKind};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Clone)]
pub struct TerminalPermissionAcceptance {
    pub schema_version: u16,
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    pub rows: Vec<([u8; 32], String)>, // Canonical coordinate digest and readable meaning, in lock order.
}

/// A package service and method rejoined to their nominal declarations in checked source.
pub struct RuntimeServicePermission {
    pub owner: PackageKeyIdentity,
    pub service_path: String,
    pub requirement_path: String,
    pub permitted: semantic_vocabulary::TerminalAuthorityDisposition,
}

impl TerminalPermissionAcceptance {
    pub fn compare_runtime_permissions(
        &self,
        package: PackageKeyIdentity,
        target: TargetProfile,
        permissions: &[RuntimeServicePermission],
        assumptions: &[crate::record::PackagePolicyComponentAssumption],
    ) -> Result<(), Vec<Diagnostic>> {
        self.check_subject(package, target)?;
        let rows =
            crate::encoding::runtime_permission_rows(package, target, permissions, assumptions)
                .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        self.compare_rows(rows)
    }

    pub fn compare_policy(&self, policy: &PackagePolicyBaseline) -> Result<(), Vec<Diagnostic>> {
        self.check_subject(policy.package(), policy.target())?;
        let (rows, _) = policy
            .acceptance_rows_with_limits(Default::default())
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot encode the runtime permission policy: {error:?}"
                ))]
            })?;
        self.compare_rows(rows)
    }

    fn check_subject(
        &self,
        package: PackageKeyIdentity,
        target: TargetProfile,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.schema_version != PACKAGE_POLICY_ROW_VERSION {
            return Err(vec![Diagnostic::error(
                "accepted runtime policy uses an obsolete schema; run omega packages update",
            )]);
        }
        if package != self.package || target != self.target {
            return Err(vec![Diagnostic::error(
                "policy package or target differs from the accepted root activation",
            )]);
        }
        Ok(())
    }

    fn compare_rows(
        &self,
        rows: Vec<crate::record::PackagePolicyRow>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut unaccepted_components = Vec::new();
        let mut rows: Vec<_> = rows
            .into_iter()
            .filter(|row| row.kind().requires_runtime_consent())
            .map(|row| {
                let component = row.kind() == PackagePolicyRowKind::ComponentAssumption;
                let key = ::wire::digest_encoder::content_identity(row.key_bytes());
                let text = row.into_canonical_text();
                if component
                    && !self
                        .rows
                        .iter()
                        .any(|accepted| accepted == &(key, text.clone()))
                {
                    unaccepted_components.push(text.clone());
                }
                (key, text)
            })
            .collect();
        // A component's environmental premise is the consumer's decision
        // (component_publication.md, "Consumer acceptance and Build
        // attenuation"); name each one the lock does not hold exactly.
        if !unaccepted_components.is_empty() {
            return Err(unaccepted_components
                .into_iter()
                .map(|meaning| {
                    Diagnostic::error(format!(
                        "the consuming project has not accepted this independent component assumption; review it with omega packages update: {}",
                        meaning.trim_end()
                    ))
                })
                .collect());
        }
        rows.sort_unstable_by_key(|(key, _)| *key);
        let mut accepted: Vec<_> = self.rows.iter().collect();
        accepted.sort_unstable_by_key(|(key, _)| *key);
        if !rows.iter().eq(accepted) {
            return Err(vec![Diagnostic::error(
                "checked runtime policy differs from accepted root Product permissions and component assumptions; run omega packages update",
            )]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::PackagePolicyRow;

    fn row(kind: PackagePolicyRowKind, key: u8, meaning: &str) -> PackagePolicyRow {
        PackagePolicyRow {
            kind,
            key_bytes: vec![key],
            canonical_bytes: Vec::new(),
            canonical_text: meaning.to_owned(),
            initial_requires_decision: true,
            audit_recommended_when_present: true,
        }
    }

    #[test]
    fn source_permission_comparison_holds_the_root_target_and_exact_method_authority() {
        use semantic_vocabulary::{TerminalAuthorityClass, TerminalAuthorityDisposition};
        let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let target = TargetProfile::LinuxX64;
        let mut acceptance = TerminalPermissionAcceptance {
            schema_version: PACKAGE_POLICY_ROW_VERSION,
            package,
            target,
            rows: vec![],
        };
        assert!(
            acceptance
                .compare_runtime_permissions(package, target, &[], &[])
                .is_ok()
        );
        let other = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        assert!(
            acceptance
                .compare_runtime_permissions(other, target, &[], &[])
                .is_err()
        );
        assert!(
            acceptance
                .compare_runtime_permissions(package, TargetProfile::WindowsX64, &[], &[])
                .is_err()
        );
        let permission = |class| RuntimeServicePermission {
            owner: other,
            service_path: "Console".into(),
            requirement_path: "Console::write".into(),
            permitted: TerminalAuthorityDisposition::from_classes([class]),
        };
        let rows = [permission(TerminalAuthorityClass::ProcessOutput)];
        assert!(
            acceptance
                .compare_runtime_permissions(package, target, &rows, &[])
                .is_err()
        );
        acceptance.rows = crate::encoding::runtime_permission_rows(package, target, &rows, &[])
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    wire::digest_encoder::content_identity(row.key_bytes()),
                    row.into_canonical_text(),
                )
            })
            .collect();
        assert!(
            acceptance
                .compare_runtime_permissions(package, target, &rows, &[])
                .is_ok()
        );
        assert!(
            acceptance
                .compare_runtime_permissions(package, target, &[], &[])
                .is_err()
        );
        assert!(
            acceptance
                .compare_runtime_permissions(
                    package,
                    target,
                    &[permission(TerminalAuthorityClass::ProcessInput)],
                    &[],
                )
                .is_err()
        );
    }

    #[test]
    fn a_component_premise_requires_the_exact_receivers_decision_and_description_rows() {
        let root = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let donor = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        let target = TargetProfile::LinuxX64;
        let mut premise = crate::record::PackagePolicyComponentAssumption {
            component: donor,
            assumption: [3; 32],
            bound_rows: vec!["physical mechanism 2 port write in verified closure a".into()],
        };
        let mut decision = TerminalPermissionAcceptance {
            schema_version: PACKAGE_POLICY_ROW_VERSION,
            package: root,
            target,
            rows: Vec::new(),
        };
        assert!(
            decision
                .compare_runtime_permissions(root, target, &[], std::slice::from_ref(&premise))
                .is_err()
        );
        decision.rows = crate::encoding::runtime_permission_rows(
            root,
            target,
            &[],
            std::slice::from_ref(&premise),
        )
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                ::wire::digest_encoder::content_identity(row.key_bytes()),
                row.into_canonical_text(),
            )
        })
        .collect();
        assert!(
            decision
                .compare_runtime_permissions(root, target, &[], std::slice::from_ref(&premise))
                .is_ok()
        );
        assert!(
            decision
                .compare_runtime_permissions(donor, target, &[], std::slice::from_ref(&premise))
                .is_err()
        );
        assert!(
            decision
                .compare_runtime_permissions(
                    root,
                    TargetProfile::WindowsX64,
                    &[],
                    std::slice::from_ref(&premise)
                )
                .is_err()
        );
        premise.bound_rows = vec!["physical mechanism 2 port write in verified closure b".into()];
        assert!(
            decision
                .compare_runtime_permissions(root, target, &[], &[premise])
                .is_err()
        );
    }

    #[test]
    fn runtime_consent_matches_each_exact_coordinate_and_meaning_across_lock_order() {
        use PackagePolicyRowKind::{ComponentAssumption, TerminalPermission};
        let permission = row(TerminalPermission, 1, "terminal permission");
        let other_permission = row(TerminalPermission, 2, "second permission");
        let component = row(ComponentAssumption, 3, "component premise");
        let lock = TerminalPermissionAcceptance {
            schema_version: PACKAGE_POLICY_ROW_VERSION,
            package: PackageKeyIdentity::from_digest([1; 32]).unwrap(),
            target: TargetProfile::host(),
            rows: [&other_permission, &component, &permission]
                .map(|r| {
                    (
                        wire::digest_encoder::content_identity(r.key_bytes()),
                        r.canonical_text().to_owned(),
                    )
                })
                .to_vec(),
        };
        let provisions = vec![
            component.clone(),
            permission.clone(),
            other_permission.clone(),
        ];
        assert!(lock.compare_rows(provisions.clone()).is_ok());
        let mut extra = provisions.clone();
        extra.push(row(PackagePolicyRowKind::Callable, 4, "regular callable"));
        assert!(lock.compare_rows(extra).is_ok());
        assert!(
            lock.compare_rows(vec![component.clone(), other_permission.clone()])
                .is_err()
        );
        for changed in [
            row(ComponentAssumption, 3, "different premise"),
            row(ComponentAssumption, 4, "component premise"),
        ] {
            let failure = lock
                .compare_rows(vec![changed.clone(), permission.clone(), other_permission.clone()])
                .unwrap_err();
            assert!(failure[0].message.contains(changed.canonical_text()));
        }
    }
}
