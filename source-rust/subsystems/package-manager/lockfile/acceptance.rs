// Retained occurrence consent and acceptance rows in `omega.lock`.
// Exact readable risk rows are opaque historical intent here; only fresh compiler output interprets their meaning, and equality never turns them into checked facts.
// Every occurrence carries its checked compilation inputs and execution context.
// `PackageAcceptanceRow` commits to an exact semantic coordinate digest plus its complete readable acceptance meaning.

use super::PackageLockError;
use crate::declarations::DependencyPurpose;
use review_evidence::permission_acceptance::TerminalPermissionAcceptance;
use review_evidence::record::{
    PACKAGE_POLICY_ROW_VERSION, PackagePolicyBaseline, PackagePolicyRow, PackagePolicyRowKind,
    PackagePolicyRowLimits, PackagePolicyRowUsage,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyOccurrence {
    pub(super) acceptance: PackagePolicyAcceptance,
    pub(super) context: super::PackageCheckedContext,
    pub(super) compilation: super::AcceptedPackageCompilation,
}

impl PackagePolicyOccurrence {
    pub(crate) fn from_review(
        review: &crate::review::CompilerIssuedPackageReview,
        compiler_identity: [u8; 32],
    ) -> Result<Self, PackageLockError> {
        if !review.preparations().is_empty() {
            return Err(PackageLockError::MultipleBuildEntries);
        }
        let context = review.checked_context();
        let mut compilation = super::AcceptedPackageCompilation::new(
            compiler_identity,
            review.semantic_bindings().to_vec(),
        )?;
        if context.purpose().is_product() {
            review
                .build_handoff()
                .append_native_bindings(context.target(), &mut compilation.native_bindings)
                .map_err(PackageLockError::NativeBindings)?;
        }
        let value = Self {
            acceptance: PackagePolicyAcceptance::from_policy(review.policy())?,
            context,
            compilation,
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

    pub const fn compilation(&self) -> &super::AcceptedPackageCompilation {
        &self.compilation
    }

    pub(super) fn validate(&self) -> Result<(), PackageLockError> {
        if self.compilation.native_bindings.iter().any(|binding| {
            self.context.purpose() != DependencyPurpose::Product
                || binding.target != self.context.target().target_name()
                || binding.supplier_package != self.acceptance.package()
                || binding.evaluation.validate().is_err()
        }) {
            return Err(PackageLockError::OccurrenceCoverage);
        }
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
    ) -> Result<(Self, PackagePolicyRowUsage), review_evidence::encoding::PackageReviewEncodingError>
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

    pub(crate) fn terminal_permissions(&self) -> TerminalPermissionAcceptance {
        TerminalPermissionAcceptance {
            schema_version: PACKAGE_POLICY_ROW_VERSION,
            package: self.package,
            target: self.target,
            rows: self
                .rows
                .iter()
                .filter(|row| row.kind.requires_runtime_consent())
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
            key: crate::digest_encoder::content_identity(row.key_bytes()),
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
