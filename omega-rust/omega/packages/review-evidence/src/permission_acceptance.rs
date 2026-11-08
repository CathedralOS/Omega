//! Refuse execution when a program requests permissions its root has not accepted.
//!
//! The package manager selects the exact root Product occurrence. A retained
//! lock is historical consent and is compared here; compilation does not write
//! one. When the consuming root's own public Build evaluation recorded an
//! assumed binding and no lock is present, the Product risk rows are projected
//! from saved declarations and admitted for this activation only. Target
//! selection and provider admission remain independent requirements.

use crate::capture::EvidenceOption;
pub use crate::capture::PreparedRuntimePolicy;
use crate::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowKind};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Completed target choices and saved declarations used for runtime consent.
/// There is no source program in this input.
pub struct RuntimePermissionInput<'a> {
    pub(crate) declarations: &'a PreparedRuntimePolicy,
    pub(crate) execution: &'a selected_dispatch::SelectedExecution,
    pub(crate) providers: &'a provider_planning::ProviderSettlement,
    pub(crate) sources: &'a package_compilation::CheckedSourceCustody,
}

impl<'a> RuntimePermissionInput<'a> {
    /// Borrow completed outputs from one compilation so callers cannot mix
    /// one program's selected code with another program's permission inputs.
    /// This assembly reads no source declarations.
    pub fn from_compilation(
        declarations: &'a PreparedRuntimePolicy,
        checked: &'a compilation_products::CheckedCompilation,
    ) -> Self {
        Self {
            declarations,
            execution: checked.execution(),
            providers: checked.providers(),
            sources: checked.sources(),
        }
    }
}

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
    pub fn from_public_build(input: &RuntimePermissionInput<'_>) -> Result<Self, Vec<Diagnostic>> {
        let package = input
            .sources
            .package_identity()
            .or_evidence("assumed binding risk rows require the consuming root package")?;
        let target = input
            .execution
            .selected_target_profile
            .or_evidence("assumed binding risk rows require an exact product target")?;
        let evaluated = input.declarations.evaluated_assumptions();
        let installed = input.execution.plans().assumed_bindings().len();
        if evaluated == 0 || evaluated != installed {
            return Err(vec![Diagnostic::error(
                "assumed binding risk rows do not match the public Build evaluation",
            )]);
        }
        let policy_rows = input.declarations.rows(input, target, package)?;
        let mut assumed_row_count = 0usize;
        let mut rows = Vec::new();
        for row in policy_rows {
            let assumed = matches!(row.kind(), PackagePolicyRowKind::AssumedBindingContract);
            if !assumed && !matches!(row.kind(), PackagePolicyRowKind::TerminalPermission) {
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

    pub fn validate(&self, input: &RuntimePermissionInput<'_>) -> Result<(), Vec<Diagnostic>> {
        if self.schema_version != PACKAGE_POLICY_ROW_VERSION {
            return Err(vec![Diagnostic::error(
                "accepted runtime policy uses an obsolete schema; run omega update",
            )]);
        }
        let rows = input.declarations.rows(input, self.target, self.package)?;
        let mut rows: Vec<_> = rows
            .into_iter()
            .filter(|row| {
                matches!(
                    row.kind(),
                    PackagePolicyRowKind::TerminalPermission
                        | PackagePolicyRowKind::AssumedBindingContract
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
