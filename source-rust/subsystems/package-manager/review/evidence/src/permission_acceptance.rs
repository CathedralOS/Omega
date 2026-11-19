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

impl TerminalPermissionAcceptance {
    pub fn compare_policy(&self, policy: &PackagePolicyBaseline) -> Result<(), Vec<Diagnostic>> {
        if self.schema_version != PACKAGE_POLICY_ROW_VERSION {
            return Err(vec![Diagnostic::error(
                "accepted runtime policy uses an obsolete schema; run omega packages update",
            )]);
        }
        if policy.package() != self.package || policy.target() != self.target {
            return Err(vec![Diagnostic::error(
                "policy package or target differs from the accepted root activation",
            )]);
        }
        let (rows, _) = policy
            .acceptance_rows_with_limits(Default::default())
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot encode the runtime permission policy: {error:?}"
                ))]
            })?;
        self.compare_rows(rows)
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
                "checked runtime policy differs from accepted root Product permissions and assumed bindings; run omega packages update",
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
    fn runtime_consent_matches_each_exact_coordinate_and_meaning_across_lock_order() {
        use PackagePolicyRowKind::{
            AssumedBindingContract, ComponentAssumption, TerminalPermission,
        };
        let permission = row(TerminalPermission, 1, "terminal permission");
        let binding = row(AssumedBindingContract, 2, "assumed contract");
        let component = row(ComponentAssumption, 3, "component premise");
        let lock = TerminalPermissionAcceptance {
            schema_version: PACKAGE_POLICY_ROW_VERSION,
            package: PackageKeyIdentity::from_digest([1; 32]).unwrap(),
            target: TargetProfile::host(),
            rows: [&binding, &component, &permission]
                .map(|r| {
                    (
                        wire::digest_encoder::content_identity(r.key_bytes()),
                        r.canonical_text().to_owned(),
                    )
                })
                .to_vec(),
        };
        let provisions = vec![component.clone(), permission.clone(), binding.clone()];
        assert!(lock.compare_rows(provisions.clone()).is_ok());
        let mut extra = provisions.clone();
        extra.push(row(PackagePolicyRowKind::Callable, 4, "regular callable"));
        assert!(lock.compare_rows(extra).is_ok());
        assert!(
            lock.compare_rows(vec![component.clone(), binding.clone()])
                .is_err()
        );
        for changed in [
            row(ComponentAssumption, 3, "different premise"),
            row(ComponentAssumption, 4, "component premise"),
        ] {
            let failure = lock
                .compare_rows(vec![changed.clone(), permission.clone(), binding.clone()])
                .unwrap_err();
            assert!(failure[0].message.contains(changed.canonical_text()));
        }
    }
}
