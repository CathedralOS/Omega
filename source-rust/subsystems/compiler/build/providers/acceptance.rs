//! Host acceptance of an exact service declaration and its allowed terminal operations.
mod permissions;
pub use permissions::ServiceTerminalAuthorityPermission;
use semantic_vocabulary::{
    PackageKeyIdentity,
    provider_identity::{ProviderPlanDigest, ServiceSchemaDigest},
};

mod service_roles;
pub use service_roles::AcceptedSemanticBindingRole;

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
