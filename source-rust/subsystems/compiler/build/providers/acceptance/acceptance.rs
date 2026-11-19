//! Host acceptance of an exact service declaration and its allowed terminal operations.
#![forbid(unsafe_code)]

mod permissions;
pub use permissions::ServiceTerminalAuthorityPermission;
use semantic_vocabulary::{
    PackageKeyIdentity,
    provider_identity::{ProviderPlanDigest, ServiceSchemaDigest},
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
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

impl AcceptedSemanticBindingRole {
    /// Stable role tag in accepted package locks and compiled review records.
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::ConsoleIo => 10,
            Self::ProcessExitExitProcessI32 => 1,
            Self::FilesystemHostService => 2,
            Self::TimeHostService => 3,
            Self::UefiX64ProgramEntry => 4,
            Self::MacosArm64ProgramEntry => 5,
            Self::MacosX64ProgramEntry => 6,
            Self::LinuxX86_64ProgramEntry => 7,
            Self::LinuxArm64ProgramEntry => 8,
            Self::WindowsX64ProgramEntry => 9,
        }
    }

    pub const fn from_canonical_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            10 => Self::ConsoleIo,
            1 => Self::ProcessExitExitProcessI32,
            2 => Self::FilesystemHostService,
            3 => Self::TimeHostService,
            4 => Self::UefiX64ProgramEntry,
            5 => Self::MacosArm64ProgramEntry,
            6 => Self::MacosX64ProgramEntry,
            7 => Self::LinuxX86_64ProgramEntry,
            8 => Self::LinuxArm64ProgramEntry,
            9 => Self::WindowsX64ProgramEntry,
            _ => return None,
        })
    }
}

/// Exact schema accepted by the receiver for this compiler-recognized role.
/// Entry slots identify the declared service independently of its selected calling plan.
pub fn accepted_service_schema_digest(
    role: AcceptedSemanticBindingRole,
    schema: &provider_plans::ServiceSchema,
) -> ServiceSchemaDigest {
    use AcceptedSemanticBindingRole::*;
    match role {
        UefiX64ProgramEntry
        | MacosArm64ProgramEntry
        | MacosX64ProgramEntry
        | LinuxX86_64ProgramEntry
        | LinuxArm64ProgramEntry
        | WindowsX64ProgramEntry => schema.declaration_identity_digest(),
        ConsoleIo | ProcessExitExitProcessI32 | FilesystemHostService | TimeHostService => {
            schema.identity_digest()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
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
            terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable::default(
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
            terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable::default(
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
                        semantic_vocabulary::TerminalAuthorityClass::ProcessInput
                            | semantic_vocabulary::TerminalAuthorityClass::ProcessOutput
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{TerminalAuthorityClass, TerminalAuthorityDisposition};

    #[test]
    fn reviewed_binding_roles_keep_their_persisted_tags() {
        use AcceptedSemanticBindingRole::*;
        let roles = [
            ProcessExitExitProcessI32,
            FilesystemHostService,
            TimeHostService,
            UefiX64ProgramEntry,
            MacosArm64ProgramEntry,
            MacosX64ProgramEntry,
            LinuxX86_64ProgramEntry,
            LinuxArm64ProgramEntry,
            WindowsX64ProgramEntry,
            ConsoleIo,
        ];
        assert_eq!(AcceptedSemanticBindingRole::from_canonical_tag(0), None);
        for (tag, role) in (1u8..).zip(roles) {
            assert_eq!(role.canonical_tag(), tag);
            assert_eq!(
                AcceptedSemanticBindingRole::from_canonical_tag(tag),
                Some(role)
            );
        }
        assert!(
            (11..=u8::MAX)
                .all(|tag| AcceptedSemanticBindingRole::from_canonical_tag(tag).is_none())
        );
    }

    #[test]
    fn entry_acceptance_compares_the_declaration_and_host_services_compare_the_full_schema() {
        use AcceptedSemanticBindingRole::*;
        use provider_plans::{ServiceMethod, ServiceSchema, ServiceSurface};

        let mut declaration = ServiceSurface::default();
        declaration.trait_name = declaration.identities.intern("Entry");
        let requirement = declaration.identities.intern("Entry::enter");
        declaration.methods.push(ServiceMethod {
            name: "enter".into(),
            requirement_owner: "Entry".into(),
            requirement_identity: requirement,
            ..Default::default()
        });
        let schema = ServiceSchema::from_declaration(std::sync::Arc::new(declaration));
        assert_ne!(
            schema.identity_digest(),
            schema.declaration_identity_digest()
        );
        for role in [
            UefiX64ProgramEntry,
            MacosArm64ProgramEntry,
            MacosX64ProgramEntry,
            LinuxX86_64ProgramEntry,
            LinuxArm64ProgramEntry,
            WindowsX64ProgramEntry,
        ] {
            assert_eq!(
                accepted_service_schema_digest(role, &schema),
                schema.declaration_identity_digest()
            );
        }
        for role in [
            ConsoleIo,
            ProcessExitExitProcessI32,
            FilesystemHostService,
            TimeHostService,
        ] {
            assert_eq!(
                accepted_service_schema_digest(role, &schema),
                schema.identity_digest()
            );
        }
    }

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
            if let Ok(binding) = result {
                let encoded = record_storage::encode(&binding, Default::default()).unwrap();
                let reopened: AcceptedSemanticBinding =
                    record_storage::decode(&encoded, Default::default()).unwrap();
                assert_eq!(binding, reopened);
                let permission = &reopened.terminal_authority_permissions()[0];
                assert_eq!(
                    reopened
                        .terminal_authority_permission_identities()
                        .text(permission.requirement_identity()),
                    Some("Console::operation")
                );
            }
        }
    }
}
