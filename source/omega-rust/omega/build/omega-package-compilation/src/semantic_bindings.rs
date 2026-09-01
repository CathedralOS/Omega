use omega_effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
use psi_core::PackageKeyIdentity;

/// One compiler consumer role whose ordinary-package declaration must be
/// accepted explicitly rather than inferred from a package or source name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AcceptedSemanticBindingRole {
    /// Target-independent recognition of the exact process-exit Console
    /// declaration. Physical lowering support remains a separate target fact.
    ConsoleExitProcessI32,
    /// Exact package-owned raw filesystem service whose use is classified as
    /// filesystem authority. This role binds the complete service schema but
    /// does not invent a provider for a requirement-only boundary.
    FilesystemHostService,
    /// Exact package-owned UEFI x86-64 application schema selected by the
    /// target's physical-entry consumer. The target still fixes the physical
    /// ABI; this binding chooses the ordinary package nominal that realizes
    /// that schema without granting the package general toolchain provenance.
    UefiX64ProgramEntry,
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
    selected_provider_plan_digest: Option<ProviderPlanDigest>,
}

impl AcceptedSemanticBinding {
    pub fn new(
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
        declaration_path: impl Into<String>,
        normalized_schema_digest: ServiceSchemaDigest,
        selected_provider_plan_digest: ProviderPlanDigest,
    ) -> Result<Self, &'static str> {
        if role != AcceptedSemanticBindingRole::ConsoleExitProcessI32 {
            return Err("accepted semantic role does not bind a selected provider plan");
        }
        let declaration_path = declaration_path.into();
        if declaration_path.is_empty() || declaration_path.chars().any(char::is_control) {
            return Err("accepted semantic binding has an invalid declaration path");
        }
        Ok(Self {
            role,
            package,
            declaration_path,
            normalized_schema_digest,
            selected_provider_plan_digest: Some(selected_provider_plan_digest),
        })
    }

    /// Bind one exact package-owned service declaration without claiming that
    /// a provider plan exists. Candidate discovery may use readable names to
    /// propose this row, but compilation consumes only the exact package,
    /// declaration path, and normalized schema identity retained here.
    pub fn new_service(
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
        declaration_path: impl Into<String>,
        normalized_schema_digest: ServiceSchemaDigest,
    ) -> Result<Self, &'static str> {
        if !matches!(
            role,
            AcceptedSemanticBindingRole::FilesystemHostService
                | AcceptedSemanticBindingRole::UefiX64ProgramEntry
        ) {
            return Err("accepted semantic role requires a selected provider plan");
        }
        let declaration_path = declaration_path.into();
        if declaration_path.is_empty() || declaration_path.chars().any(char::is_control) {
            return Err("accepted semantic binding has an invalid declaration path");
        }
        Ok(Self {
            role,
            package,
            declaration_path,
            normalized_schema_digest,
            selected_provider_plan_digest: None,
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

    pub const fn selected_provider_plan_digest(&self) -> Option<ProviderPlanDigest> {
        self.selected_provider_plan_digest
    }
}

/// Derive the exact schema commitment owned by one accepted semantic role.
///
/// UEFI target calling-plan applications are independently selected, replayed,
/// and retained by the target entry contract. Omitting those two derived
/// fields here lets semantic-only candidate review and target compilation bind
/// the same authored nominal/schema without creating a second ABI authority.
#[doc(hidden)]
pub fn accepted_service_schema_digest(
    role: AcceptedSemanticBindingRole,
    schema: &omega_effects::provider_plan::ServiceSchema,
) -> ServiceSchemaDigest {
    if role != AcceptedSemanticBindingRole::UefiX64ProgramEntry {
        return schema.identity_digest();
    }
    let mut semantic = schema.clone();
    for method in &mut semantic.methods {
        method.calling_plan_report_fingerprint = None;
        method.calling_plan_commitment = None;
    }
    semantic.identity_digest()
}
