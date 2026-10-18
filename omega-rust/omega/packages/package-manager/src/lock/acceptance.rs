//! Retained consent, not a serialized compiler analysis.
//!
//! Exact readable risk rows are opaque historical intent here. Only fresh compiler
//! output interprets their meaning; equality never turns them into checked facts.

use super::PackageLockError;
use crate::declarations::DependencyPurpose;
use package_evidence::record::{
    PACKAGE_POLICY_ROW_VERSION, PackagePolicyBaseline, PackagePolicyRow, PackagePolicyRowKind,
    PackagePolicyRowLimits, PackagePolicyRowUsage,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Independently retained consent for one exact checked package occurrence.
/// The execution profile is recorded context, never inferred from this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyOccurrence {
    pub(super) acceptance: PackagePolicyAcceptance,
    pub(super) context: super::PackageCheckedContext,
    // Version 3 records remain usable as review baselines, not compile inputs.
    pub(super) compilation: Option<super::AcceptedPackageCompilation>,
}

impl PackagePolicyOccurrence {
    pub(crate) fn from_policy(
        policy: &PackagePolicyBaseline,
        context: super::PackageCheckedContext,
        compiler_identity: [u8; 32],
        semantic_bindings: &[package_compilation::AcceptedSemanticBinding],
    ) -> Result<Self, PackageLockError> {
        let value = Self {
            acceptance: PackagePolicyAcceptance::from_policy(policy)?,
            context,
            compilation: Some(super::AcceptedPackageCompilation::new(
                compiler_identity,
                semantic_bindings.to_vec(),
            )?),
        };
        value.validate()?;
        Ok(value)
    }

    pub const fn acceptance(&self) -> &PackagePolicyAcceptance {
        &self.acceptance
    }
    pub const fn context(&self) -> super::PackageCheckedContext {
        self.context
    }

    /// Absent for historical lock or consent schemas. Fresh review must
    /// supply these inputs before compilation or publication can use the record.
    pub const fn compilation(&self) -> Option<&super::AcceptedPackageCompilation> {
        if self.acceptance.schema_version == PACKAGE_POLICY_ROW_VERSION {
            self.compilation.as_ref()
        } else {
            None
        }
    }

    pub(super) fn validate(&self) -> Result<(), PackageLockError> {
        if self.context.target() != self.acceptance.target()
            || (self.context.purpose() == DependencyPurpose::Build
                && self.context.build_execution_profile() != Some(self.acceptance.target()))
        {
            return Err(PackageLockError::TargetMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyAcceptance {
    pub(super) schema_version: u16,
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
                schema_version: PACKAGE_POLICY_ROW_VERSION,
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

    /// Pass the selected occurrence's consent into compilation. The compiler
    /// compares it with fresh checked service declarations before production.
    pub(crate) fn terminal_permissions(
        &self,
    ) -> package_evidence::permission_acceptance::TerminalPermissionAcceptance {
        package_evidence::permission_acceptance::TerminalPermissionAcceptance {
            schema_version: self.schema_version,
            package: self.package,
            target: self.target,
            rows: self
                .rows
                .iter()
                .filter(|row| {
                    matches!(
                        row.kind,
                        PackagePolicyRowKind::TerminalPermission
                            | PackagePolicyRowKind::AssumedBindingContract
                    )
                })
                .map(|row| (row.key, row.text.clone()))
                .collect(),
        }
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
pub(crate) struct PackageAcceptanceRow {
    pub(super) kind: PackagePolicyRowKind,
    pub(super) key: [u8; 32],
    pub(super) text: String,
}

impl PackageAcceptanceRow {
    fn from_row(row: PackagePolicyRow) -> Self {
        Self {
            kind: row.kind(),
            key: crate::digest_encoder::content_identity(&row.key_bytes()).into(),
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
