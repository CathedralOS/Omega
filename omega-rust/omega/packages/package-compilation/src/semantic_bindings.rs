// Explicit consumer-policy bindings for ordinary-package declarations that
// realize compiler-recognized semantic roles (`Console`, `ProcessExit`,
// `FilesystemHostService`, `TimeHostService`, and per-target `ProgramEntry`).
// Constructing `AcceptedSemanticBinding` records the policy authority supplied
// to compilation without inferring provenance from package or source names; the
// compiler still rejoins package owner, declaration path, normalized schema,
// provider plan, and terminal-authority permissions before admitting use.

use effects::ServiceTerminalAuthorityPermission;
use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
use semantic_vocabulary::PackageKeyIdentity;

pub fn program_entry_semantic_binding_role(
    package: target::ProgramEntryPhysicalContractPackage,
) -> AcceptedSemanticBindingRole {
    match package {
        target::ProgramEntryPhysicalContractPackage::UefiX64 => {
            AcceptedSemanticBindingRole::UefiX64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::MacosArm64 => {
            AcceptedSemanticBindingRole::MacosArm64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::MacosX64 => {
            AcceptedSemanticBindingRole::MacosX64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64 => {
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::LinuxArm64 => {
            AcceptedSemanticBindingRole::LinuxArm64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::WindowsX64 => {
            AcceptedSemanticBindingRole::WindowsX64ProgramEntry
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum AcceptedSemanticBindingRole {
    ConsoleIo, // Explicit Console input/output policy; grants no process termination.
    ProcessExitExitProcessI32, // Target-independent recognition of the canonical core `ProcessExit` boundary requirement realized by the bound package's provider.
    FilesystemHostService, // Exact package-owned raw filesystem service classified as filesystem authority.
    TimeHostService,       // Exact package-owned raw clock service.
    UefiX64ProgramEntry,   // Exact package-owned UEFI x86-64 application schema.
    MacosArm64ProgramEntry, // Exact package-owned macOS ARM64 application schema.
    MacosX64ProgramEntry,  // Exact package-owned macOS x86-64 application schema.
    LinuxX86_64ProgramEntry, // Exact package-owned Linux x86-64 application schema.
    LinuxArm64ProgramEntry, // Exact package-owned Linux ARM64 application schema.
    WindowsX64ProgramEntry, // Exact package-owned Windows x86-64 application schema.
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct AcceptedSemanticBinding {
    role: AcceptedSemanticBindingRole,
    package: PackageKeyIdentity,
    declaration_path: String,
    normalized_schema_digest: ServiceSchemaDigest,
    selected_provider_plan_digest: Option<ProviderPlanDigest>,
    terminal_authority_permissions: Vec<ServiceTerminalAuthorityPermission>,
    terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable, // Identity table the permission rows' `requirement_identity` indices resolve against.
}

impl AcceptedSemanticBinding {
    pub fn new(
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
        declaration_path: impl Into<String>,
        normalized_schema_digest: ServiceSchemaDigest,
        selected_provider_plan_digest: ProviderPlanDigest,
    ) -> Result<Self, &'static str> {
        if !matches!(
            role,
            AcceptedSemanticBindingRole::ConsoleIo
                | AcceptedSemanticBindingRole::ProcessExitExitProcessI32
        ) {
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
            terminal_authority_permissions: Vec::new(),
            terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable::from(
                Vec::new(),
            ),
        })
    }

    pub fn new_service(
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
        declaration_path: impl Into<String>,
        normalized_schema_digest: ServiceSchemaDigest,
    ) -> Result<Self, &'static str> {
        if !matches!(
            role,
            AcceptedSemanticBindingRole::FilesystemHostService
                | AcceptedSemanticBindingRole::TimeHostService
                | AcceptedSemanticBindingRole::UefiX64ProgramEntry
                | AcceptedSemanticBindingRole::MacosArm64ProgramEntry
                | AcceptedSemanticBindingRole::MacosX64ProgramEntry
                | AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry
                | AcceptedSemanticBindingRole::LinuxArm64ProgramEntry
                | AcceptedSemanticBindingRole::WindowsX64ProgramEntry
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
            terminal_authority_permissions: Vec::new(),
            terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable::from(
                Vec::new(),
            ),
        })
    }

    pub fn with_terminal_authority_permissions(
        mut self,
        mut permissions: Vec<ServiceTerminalAuthorityPermission>,
        permission_identities: &terminal_psi::TerminalIdentityTable,
    ) -> Result<Self, &'static str> {
        if self.role == AcceptedSemanticBindingRole::ConsoleIo
            && permissions.iter().any(|permission| {
                permission.permitted().classes().iter().any(|class| {
                    !matches!(
                        class,
                        effects::TerminalAuthorityClass::ProcessInput
                            | effects::TerminalAuthorityClass::ProcessOutput
                    )
                })
            })
        {
            return Err("Console I/O acceptance grants only process input and output");
        }
        if permissions
            .iter()
            .any(|permission| permission.service_schema() != self.normalized_schema_digest)
        {
            return Err("terminal-authority permission names a different service schema");
        }
        if permissions.iter().any(|permission| {
            permission_identities
                .text(permission.requirement_identity())
                .is_none_or(|text| text.is_empty() || text.chars().any(char::is_control))
        }) {
            return Err("terminal-authority permission has an invalid requirement identity");
        }
        permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(right.service_schema().as_bytes())
                .then_with(|| {
                    permission_identities
                        .text(left.requirement_identity())
                        .cmp(&permission_identities.text(right.requirement_identity()))
                })
        });
        if permissions.windows(2).any(|rows| {
            rows[0].service_schema() == rows[1].service_schema()
                && permission_identities.text(rows[0].requirement_identity())
                    == permission_identities.text(rows[1].requirement_identity())
        }) {
            return Err("terminal-authority permissions repeat an exact requirement");
        }
        let mut identities = self
            .terminal_authority_permission_identities
            .clone()
            .into_builder();
        for permission in &mut permissions {
            permission.rekey_requirement_identity(
                identities.intern(
                    permission_identities
                        .text(permission.requirement_identity())
                        .unwrap_or_default(),
                ),
            );
        }
        self.terminal_authority_permissions = permissions;
        self.terminal_authority_permission_identities = identities.build();
        Ok(self)
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

    pub fn terminal_authority_permissions(&self) -> &[ServiceTerminalAuthorityPermission] {
        &self.terminal_authority_permissions
    }

    pub const fn terminal_authority_permission_identities(
        &self,
    ) -> &terminal_psi::TerminalIdentityTable {
        &self.terminal_authority_permission_identities
    }
}

#[doc(hidden)]
pub fn accepted_service_schema_digest(
    role: AcceptedSemanticBindingRole,
    schema: &effects::provider_plan::ServiceSchema,
) -> ServiceSchemaDigest {
    if !matches!(
        role,
        AcceptedSemanticBindingRole::UefiX64ProgramEntry
            | AcceptedSemanticBindingRole::MacosArm64ProgramEntry
            | AcceptedSemanticBindingRole::MacosX64ProgramEntry
            | AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry
            | AcceptedSemanticBindingRole::LinuxArm64ProgramEntry
            | AcceptedSemanticBindingRole::WindowsX64ProgramEntry
    ) {
        return schema.identity_digest();
    }
    schema.declaration_identity_digest()
}

#[cfg(test)]
mod tests {
    use super::*;
    use effects::{TerminalAuthorityClass, TerminalAuthorityDisposition};

    #[test]
    fn console_io_permission_cannot_grant_process_exit() {
        let package = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let schema = ServiceSchemaDigest::from_digest([3; 32]);
        let binding = AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleIo,
            package,
            "Console",
            schema,
            ProviderPlanDigest::from_digest([5; 32]),
        )
        .unwrap();
        for (class, accepted) in [
            (TerminalAuthorityClass::ProcessInput, true),
            (TerminalAuthorityClass::ProcessOutput, true),
            (TerminalAuthorityClass::ProcessTermination, false),
        ] {
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
            let permission = ServiceTerminalAuthorityPermission::new(
                schema,
                identities.intern("Console::operation"),
                TerminalAuthorityDisposition::from_classes([class]),
            );
            let result = binding
                .clone()
                .with_terminal_authority_permissions(vec![permission], &identities.build());
            assert_eq!(result.is_ok(), accepted, "authority class {class:?}");
        }
    }
}
