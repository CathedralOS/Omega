//! Retained consent, not a serialized compiler analysis.
//!
//! Exact readable risk rows are opaque historical intent here. Only fresh compiler
//! output interprets their meaning; equality never turns them into checked facts.

use super::PackageLockError;
use package_evidence::record::{
    PackagePolicyBaseline, PackagePolicyRow, PackagePolicyRowKind, PackagePolicyRowLimits,
    PackagePolicyRowUsage,
};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyAcceptance {
    pub(super) package: PackageKeyIdentity,
    pub(super) target: TargetProfile,
    pub(super) rows: Vec<PackageAcceptanceRow>,
}

impl PackagePolicyAcceptance {
    pub fn from_policy(policy: &PackagePolicyBaseline) -> Result<Self, PackageLockError> {
        Self::project(policy, PackagePolicyRowLimits::default())
            .map(|(value, _)| value)
            .map_err(PackageLockError::Encoding)
    }

    pub(crate) fn project(
        policy: &PackagePolicyBaseline,
        limits: PackagePolicyRowLimits,
    ) -> Result<(Self, PackagePolicyRowUsage), package_evidence::encoding::PackageReviewEncodingError>
    {
        let (rows, usage) = policy.acceptance_rows_with_limits(limits)?;
        let mut rows: Vec<_> = rows
            .into_iter()
            .map(PackageAcceptanceRow::from_row)
            .collect();
        rows.sort_unstable_by_key(|row| (row.kind, row.key));
        Ok((
            Self {
                package: policy.package(),
                target: policy.target(),
                rows,
            },
            usage,
        ))
    }

    pub fn package(&self) -> PackageKeyIdentity {
        self.package
    }
    pub fn target(&self) -> TargetProfile {
        self.target
    }
    pub fn rows(&self) -> &[PackageAcceptanceRow] {
        &self.rows
    }
    pub(crate) fn into_rows(self) -> Vec<PackageAcceptanceRow> {
        self.rows
    }

    pub fn canonical_text(&self) -> Result<String, PackageLockError> {
        super::text::acceptance::write(self)
    }
}

/// A commitment to an exact semantic coordinate plus its complete readable
/// acceptance meaning. The digest shortens repeated signature-shaped keys; it
/// does not replace the meaning or authorize a different reconstructed row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageAcceptanceRow {
    pub(super) kind: PackagePolicyRowKind,
    pub(super) key: [u8; 32],
    pub(super) text: String,
}

impl PackageAcceptanceRow {
    fn from_row(row: PackagePolicyRow) -> Self {
        Self {
            kind: row.kind(),
            key: Sha256::digest(row.key_bytes()).into(),
            text: row.into_canonical_text(),
        }
    }
    pub fn kind(&self) -> PackagePolicyRowKind {
        self.kind
    }
    pub fn key_bytes(&self) -> &[u8] {
        &self.key
    }
    pub fn canonical_text(&self) -> &str {
        &self.text
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }
    pub fn initial_requires_decision(&self) -> bool {
        true
    }
    pub fn update_requires_decision(&self) -> bool {
        true
    }
    pub fn audit_recommended_when_present(&self) -> bool {
        true
    }
    pub fn audit_recommended_on_change(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::text::acceptance;

    fn sample() -> PackagePolicyAcceptance {
        PackagePolicyAcceptance {
            package: PackageKeyIdentity::from_digest([7; 32]).unwrap(),
            target: TargetProfile::WindowsX64,
            rows: vec![PackageAcceptanceRow {
                kind: PackagePolicyRowKind::DangerousCapability,
                key: [3; 32],
                text: "field value {\n  permission filesystem_metadata_query\n}\n".into(),
            }],
        }
    }

    #[test]
    fn compact_acceptance_roundtrips_exact_meaning_and_bounds_storage() {
        let policy = sample();
        let text = policy.canonical_text().unwrap();
        let (recovered, owned) =
            acceptance::read(&text, policy.package, policy.target, 1, 4096).unwrap();
        assert_eq!(policy, recovered);
        assert!(acceptance::read(&text, policy.package, policy.target, 0, 4096).is_err());
        assert!(acceptance::read(&text, policy.package, policy.target, 1, owned - 1).is_err());
        for bad in [
            text.replace("acceptance_schema 2", "acceptance_schema 99"),
            text.replace("dangerous_capability", "semantic_dependency"),
            text.replace("rows 1", "rows 999999999"),
            text.replace("meaning ", "meaning 0"),
            format!("{text}trailing\n"),
            text[..text.len() - 1].to_owned(),
        ] {
            assert!(acceptance::read(&bad, policy.package, policy.target, 100, 4096).is_err());
        }
        // Opaque historical text is not interpreted as a finding. Changing its
        // meaning changes equality against freshly reconstructed acceptance.
        let changed = text.replace("metadata_query", "namespace_edit");
        let (changed, _) =
            acceptance::read(&changed, policy.package, policy.target, 1, 4096).unwrap();
        assert_ne!(policy, changed);
    }
}
