use omega_effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
use psi_core::PackageKeyIdentity;

/// One compiler consumer role whose ordinary-package declaration must be
/// accepted explicitly rather than inferred from a package or source name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AcceptedSemanticBindingRole {
    LinuxConsoleExitGroupI32,
}

/// Consumer-policy acceptance of one exact package-owned semantic surface.
///
/// This row is authority supplied to compilation; constructing it does not
/// prove that a human or model audited anything. The compiler still rejoins
/// the package owner, nominal declaration, normalized schema, complete
/// selected provider plan, target, and intrinsic ABI before using the role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedSemanticBinding {
    role: AcceptedSemanticBindingRole,
    package: PackageKeyIdentity,
    declaration_path: String,
    normalized_schema_digest: ServiceSchemaDigest,
    selected_provider_plan_digest: ProviderPlanDigest,
}

impl AcceptedSemanticBinding {
    pub fn new(
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
        declaration_path: impl Into<String>,
        normalized_schema_digest: ServiceSchemaDigest,
        selected_provider_plan_digest: ProviderPlanDigest,
    ) -> Result<Self, &'static str> {
        let declaration_path = declaration_path.into();
        if declaration_path.is_empty() || declaration_path.chars().any(char::is_control) {
            return Err("accepted semantic binding has an invalid declaration path");
        }
        Ok(Self {
            role,
            package,
            declaration_path,
            normalized_schema_digest,
            selected_provider_plan_digest,
        })
    }

    pub const fn role(&self) -> AcceptedSemanticBindingRole {
        self.role
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub fn declaration_path(&self) -> &str {
        &self.declaration_path
    }

    pub const fn normalized_schema_digest(&self) -> ServiceSchemaDigest {
        self.normalized_schema_digest
    }

    pub const fn selected_provider_plan_digest(&self) -> ProviderPlanDigest {
        self.selected_provider_plan_digest
    }
}
