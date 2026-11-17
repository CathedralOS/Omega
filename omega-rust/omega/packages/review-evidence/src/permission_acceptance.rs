/*
Runtime permission and assumed-binding consent verification for the root Product
occurrence (`RuntimePermissionInput` and `TerminalPermissionAcceptance`).

The package manager selects the exact root Product occurrence and compares its
projected `TerminalPermission` and `AssumedBindingContract` policy rows against
retained lock consent without writing a lock during compilation. Only the lock
carries accepted rows: the root's own `Build` authorization is not acceptance,
so the compiler refuses a runtime Product with assumed bindings when no lock
exists. Target selection and provider admission remain independent requirements.
*/

pub use crate::capture::PreparedRuntimePolicy;
use crate::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowKind};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub struct RuntimePermissionInput<'a> {
    pub(crate) declarations: &'a PreparedRuntimePolicy,
    pub(crate) compilation: &'a products::CheckedCompilation,
}

impl<'a> RuntimePermissionInput<'a> {
    pub fn from_compilation(
        declarations: &'a PreparedRuntimePolicy,
        checked: &'a products::CheckedCompilation,
    ) -> Self {
        Self {
            declarations,
            compilation: checked,
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
