/*
Runtime permission and assumed-binding consent verification for the root Product
occurrence (`RuntimePermissionInput` and `TerminalPermissionAcceptance`).

The package manager selects the exact root Product occurrence and compares its
projected `TerminalPermission` and `AssumedBindingContract` policy rows against
retained lock consent without writing a lock during compilation. When no lock is
present and the consuming root's own public `Build` evaluation recorded an
assumed binding, `TerminalPermissionAcceptance::from_public_build` projects the
Product risk rows from `PreparedRuntimePolicy` saved before shared Terminal
publication and admits them for that single activation only. Target selection
and provider admission remain independent requirements.
*/

pub use crate::capture::PreparedRuntimePolicy;
use crate::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowKind};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub struct RuntimePermissionInput<'a> {
    pub(crate) declarations: &'a PreparedRuntimePolicy,
    pub(crate) execution: &'a selected_dispatch::SelectedExecution,
    pub(crate) sources: &'a package_compilation::CheckedSourceCustody,
}

impl<'a> RuntimePermissionInput<'a> {
    pub fn from_compilation(
        declarations: &'a PreparedRuntimePolicy,
        checked: &'a compilation_products::CheckedCompilation,
    ) -> Self {
        Self {
            declarations,
            execution: checked.execution(),
            sources: checked.sources(),
        }
    }
}

#[derive(Clone)]
pub struct TerminalPermissionAcceptance {
    pub schema_version: u16,
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    pub rows: Vec<([u8; 32], String)>, // Canonical coordinate digest and readable meaning, in lock order.
}

impl TerminalPermissionAcceptance {
    pub fn from_public_build(input: &RuntimePermissionInput<'_>) -> Result<Self, Vec<Diagnostic>> {
        let package = input.sources.package_identity().ok_or_else(|| {
            vec![Diagnostic::error(
                "assumed binding risk rows require the consuming root package",
            )]
        })?;
        let target = input.execution.target_profile().ok_or_else(|| {
            vec![Diagnostic::error(
                "assumed binding risk rows require an exact product target",
            )]
        })?;
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
                "accepted runtime policy uses an obsolete schema; run omega packages update",
            )]);
        }
        let rows = input.declarations.rows(input, self.target, self.package)?;
        let mut unaccepted_components = Vec::new();
        let mut rows: Vec<_> = rows
            .into_iter()
            .filter(|row| {
                matches!(
                    row.kind(),
                    PackagePolicyRowKind::TerminalPermission
                        | PackagePolicyRowKind::AssumedBindingContract
                        | PackagePolicyRowKind::ComponentAssumption
                )
            })
            .map(|row| {
                let component = row.kind() == PackagePolicyRowKind::ComponentAssumption;
                let key = optimization_core::digest_encoder::content_identity(row.key_bytes());
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
        if rows != self.rows {
            return Err(vec![Diagnostic::error(
                "checked runtime policy differs from accepted root Product permissions and assumed bindings; run omega packages update",
            )]);
        }
        Ok(())
    }
}
