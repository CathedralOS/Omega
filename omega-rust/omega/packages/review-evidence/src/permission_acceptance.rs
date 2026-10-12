//! Match retained permission consent against fresh checked declarations.
//!
//! The package manager selects the exact root Product occurrence. Compilation
//! consumes its accepted rows as input; the caller need not receive checked
//! trees just to compare those rows. Consent is not provider evidence, and this
//! projection never reconstructs the rest of a package review.

use crate::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowLimits};
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Exact historical consent, not an authorization minted by compilation.
pub struct TerminalPermissionAcceptance {
    pub schema_version: u16,
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    /// Canonical coordinate digest and readable meaning, in lock order.
    pub rows: Vec<([u8; 32], String)>,
}

impl TerminalPermissionAcceptance {
    pub fn validate(&self, checked: &CheckedCompilation) -> Result<(), Vec<Diagnostic>> {
        if self.schema_version != PACKAGE_POLICY_ROW_VERSION {
            return Err(vec![Diagnostic::error(
                "accepted terminal permissions use an obsolete schema; run omega update",
            )]);
        }
        let permissions =
            crate::project_checked_terminal_permission_policy(checked, self.target, self.package)?;
        let (rows, _) = permissions
            .acceptance_rows_with_limits(PackagePolicyRowLimits::default())
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let mut rows: Vec<_> = rows
            .into_iter()
            .map(|row| {
                let key = optimization_core::digest_encoder::content_identity(row.key_bytes());
                (key, row.into_canonical_text())
            })
            .collect();
        rows.sort_unstable_by_key(|(key, _)| *key);
        if rows != self.rows {
            return Err(vec![Diagnostic::error(
                "checked terminal permissions differ from accepted root Product permissions; run omega update",
            )]);
        }
        Ok(())
    }
}
