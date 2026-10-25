//! Match root runtime-policy consent against fresh checked declarations.
//!
//! The package manager selects the exact root Product occurrence. A retained
//! lock is historical consent and is compared here; compilation does not write
//! one. When the consuming root's own public Build evaluation recorded an
//! assumed binding and no lock is present, the Product risk rows are projected
//! from that evaluation and admitted for this activation only. Consent is not
//! provider evidence, and this projection never reconstructs the rest of a
//! package review.

use crate::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowLimits};
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Exact consent for one root Product occurrence.
///
/// A lock supplies historical rows. [`Self::from_public_build`] projects the
/// same rows from the compilation whose Build evaluation recorded the
/// relationship. Neither form writes a lock or accepts a dependency's
/// relationship on another package's behalf.
pub struct TerminalPermissionAcceptance {
    pub schema_version: u16,
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    /// Canonical coordinate digest and readable meaning, in lock order.
    pub rows: Vec<([u8; 32], String)>,
}

impl TerminalPermissionAcceptance {
    /// Product risk rows carried by the root Build that this compilation
    /// evaluated. The rows exist only when that evaluation recorded the
    /// assumed relationship and the checked plan still names the same count.
    /// A later locked compile does not reuse this value; it must present its
    /// own retained rows.
    pub fn from_public_build(checked: &CheckedCompilation) -> Result<Self, Vec<Diagnostic>> {
        let package = checked.sources().package_identity().ok_or_else(|| {
            vec![Diagnostic::error(
                "assumed binding risk rows require the consuming root package",
            )]
        })?;
        let target = checked.execution().selected_target_profile.ok_or_else(|| {
            vec![Diagnostic::error(
                "assumed binding risk rows require an exact product target",
            )]
        })?;
        let evaluated = checked.build().config.assumed_bindings.len();
        let installed = checked
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .len();
        if evaluated == 0 || evaluated != installed {
            return Err(vec![Diagnostic::error(
                "assumed binding risk rows do not match the public Build evaluation",
            )]);
        }
        let policy = crate::project_checked_package_policy(checked, target, package)?;
        let (policy_rows, _) = policy
            .acceptance_rows_with_limits(PackagePolicyRowLimits::default())
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let mut assumed_row_count = 0usize;
        let mut rows = Vec::new();
        for row in policy_rows {
            let assumed = matches!(
                row.kind(),
                crate::record::PackagePolicyRowKind::AssumedBindingContract
            );
            if !assumed
                && !matches!(
                    row.kind(),
                    crate::record::PackagePolicyRowKind::TerminalPermission
                )
            {
                continue;
            }
            if assumed {
                assumed_row_count += 1;
            }
            let key = optimization_core::digest_encoder::content_identity(row.key_bytes());
            rows.push((key, row.into_canonical_text()));
        }
        if assumed_row_count != evaluated {
            return Err(vec![Diagnostic::error(
                "public Build evaluation produced no matching assumed-binding Product risk rows",
            )]);
        }
        rows.sort_unstable_by_key(|(key, _)| *key);
        Ok(Self {
            schema_version: PACKAGE_POLICY_ROW_VERSION,
            package,
            target,
            rows,
        })
    }

    pub fn validate(&self, checked: &CheckedCompilation) -> Result<(), Vec<Diagnostic>> {
        if self.schema_version != PACKAGE_POLICY_ROW_VERSION {
            return Err(vec![Diagnostic::error(
                "accepted runtime policy uses an obsolete schema; run omega update",
            )]);
        }
        let (rows, _) = if checked
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .is_empty()
        {
            crate::project_checked_terminal_permission_policy(checked, self.target, self.package)?
                .acceptance_rows_with_limits(PackagePolicyRowLimits::default())
        } else {
            crate::project_checked_package_policy(checked, self.target, self.package)?
                .acceptance_rows_with_limits(PackagePolicyRowLimits::default())
        }
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let mut rows: Vec<_> = rows
            .into_iter()
            .filter(|row| {
                matches!(
                    row.kind(),
                    crate::record::PackagePolicyRowKind::TerminalPermission
                        | crate::record::PackagePolicyRowKind::AssumedBindingContract
                )
            })
            .map(|row| {
                let key = optimization_core::digest_encoder::content_identity(row.key_bytes());
                (key, row.into_canonical_text())
            })
            .collect();
        rows.sort_unstable_by_key(|(key, _)| *key);
        if rows != self.rows {
            return Err(vec![Diagnostic::error(
                "checked runtime policy differs from accepted root Product permissions and assumed bindings; run omega update",
            )]);
        }
        Ok(())
    }
}
