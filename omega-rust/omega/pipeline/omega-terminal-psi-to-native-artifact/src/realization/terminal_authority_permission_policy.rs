//! Receiving permission policy for D45's service/terminal containment join.

pub use omega_effects::ServiceTerminalAuthorityPermission as TerminalAuthorityPermissionPolicyRow;
use omega_effects::{
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,
    provider_plan::ServiceSchemaDigest,
};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION: u32 = 1;

/// Exact receiving policy accepted for one native realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPermissionPolicy {
    identity: TerminalAuthorityPermissionPolicyIdentity,
    rows: Vec<TerminalAuthorityPermissionPolicyRow>,
}

impl TerminalAuthorityPermissionPolicy {
    pub const fn identity(&self) -> TerminalAuthorityPermissionPolicyIdentity {
        self.identity
    }

    pub fn rows(&self) -> &[TerminalAuthorityPermissionPolicyRow] {
        &self.rows
    }

    pub fn permission_for(
        &self,
        service_schema: ServiceSchemaDigest,
        requirement_identity: &str,
    ) -> Result<TerminalAuthorityDisposition, MissingTerminalAuthorityPermission> {
        self.rows
            .iter()
            .find(|row| {
                row.service_schema() == service_schema
                    && row.requirement_identity() == requirement_identity
            })
            .map(|row| row.permitted().clone())
            .ok_or_else(|| MissingTerminalAuthorityPermission {
                service_schema,
                requirement_identity: requirement_identity.to_owned(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingTerminalAuthorityPermission {
    service_schema: ServiceSchemaDigest,
    requirement_identity: String,
}

impl MissingTerminalAuthorityPermission {
    pub const fn service_schema(&self) -> ServiceSchemaDigest {
        self.service_schema
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAuthorityPermissionPolicyBuildError {
    EmptyRequirement,
    InvalidRequirement,
    DuplicatePermission {
        service_schema: ServiceSchemaDigest,
        requirement_identity: String,
    },
}

pub fn terminal_authority_permission_policy_with_rows(
    mut rows: Vec<TerminalAuthorityPermissionPolicyRow>,
) -> Result<TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError> {
    if rows.iter().any(|row| row.requirement_identity().is_empty()) {
        return Err(TerminalAuthorityPermissionPolicyBuildError::EmptyRequirement);
    }
    if rows
        .iter()
        .any(|row| row.requirement_identity().chars().any(char::is_control))
    {
        return Err(TerminalAuthorityPermissionPolicyBuildError::InvalidRequirement);
    }
    rows.sort_by(|left, right| {
        left.service_schema()
            .as_bytes()
            .cmp(right.service_schema().as_bytes())
            .then_with(|| {
                left.requirement_identity()
                    .cmp(right.requirement_identity())
            })
    });
    if let Some(rows) = rows.windows(2).find(|rows| {
        rows[0].service_schema() == rows[1].service_schema()
            && rows[0].requirement_identity() == rows[1].requirement_identity()
    }) {
        return Err(
            TerminalAuthorityPermissionPolicyBuildError::DuplicatePermission {
                service_schema: rows[0].service_schema(),
                requirement_identity: rows[0].requirement_identity().to_owned(),
            },
        );
    }
    let identity = permission_policy_identity(&rows);
    Ok(TerminalAuthorityPermissionPolicy { identity, rows })
}

/// Empty, exact deny-by-absence policy. Authority-bearing and authority-free
/// terminal leaves both require explicit schema/requirement permission rows.
pub fn current_terminal_authority_permission_policy() -> TerminalAuthorityPermissionPolicy {
    static IDENTITY: OnceLock<TerminalAuthorityPermissionPolicyIdentity> = OnceLock::new();
    TerminalAuthorityPermissionPolicy {
        identity: *IDENTITY.get_or_init(|| permission_policy_identity(&[])),
        rows: Vec::new(),
    }
}

fn permission_policy_identity(
    rows: &[TerminalAuthorityPermissionPolicyRow],
) -> TerminalAuthorityPermissionPolicyIdentity {
    let mut digest = Sha256::new();
    digest.update(b"omega.terminal-authority.permission-policy.v1\0");
    digest.update(TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION.to_be_bytes());
    digest.update((rows.len() as u64).to_be_bytes());
    for row in rows {
        digest.update(row.service_schema().as_bytes());
        digest.update((row.requirement_identity().len() as u64).to_be_bytes());
        digest.update(row.requirement_identity().as_bytes());
        digest.update((row.permitted().classes().len() as u64).to_be_bytes());
        for class in row.permitted().classes() {
            digest.update([class.canonical_tag()]);
        }
    }
    TerminalAuthorityPermissionPolicyIdentity::from_parts(
        TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
        digest.finalize().into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::{TerminalAuthorityClass, provider_plan::ServiceSchema};

    fn schema(marker: &str) -> ServiceSchemaDigest {
        ServiceSchema {
            trait_name: marker.to_owned(),
            ..ServiceSchema::default()
        }
        .identity_digest()
    }

    #[test]
    fn rows_are_canonical_and_exact() {
        let first = schema("First");
        let second = schema("Second");
        let policy = terminal_authority_permission_policy_with_rows(vec![
            TerminalAuthorityPermissionPolicyRow::new(
                second,
                "Second::read",
                TerminalAuthorityDisposition::from_classes([]),
            ),
            TerminalAuthorityPermissionPolicyRow::new(
                first,
                "First::exit",
                TerminalAuthorityDisposition::from_classes([
                    TerminalAuthorityClass::ProcessTermination,
                ]),
            ),
        ])
        .expect("exact rows");
        assert_eq!(policy.rows()[0].service_schema(), first);
        assert_eq!(
            policy
                .permission_for(first, "First::exit")
                .expect("exact permission")
                .classes(),
            &[TerminalAuthorityClass::ProcessTermination]
        );
        assert!(policy.permission_for(first, "Second::read").is_err());
    }

    #[test]
    fn duplicate_and_empty_requirements_reject() {
        let schema = schema("Only");
        let row = TerminalAuthorityPermissionPolicyRow::new(
            schema,
            "Only::call",
            TerminalAuthorityDisposition::from_classes([]),
        );
        assert!(matches!(
            terminal_authority_permission_policy_with_rows(vec![row.clone(), row]),
            Err(TerminalAuthorityPermissionPolicyBuildError::DuplicatePermission { .. })
        ));
        assert_eq!(
            terminal_authority_permission_policy_with_rows(vec![
                TerminalAuthorityPermissionPolicyRow::new(
                    schema,
                    "",
                    TerminalAuthorityDisposition::from_classes([]),
                ),
            ]),
            Err(TerminalAuthorityPermissionPolicyBuildError::EmptyRequirement)
        );
        assert_eq!(
            terminal_authority_permission_policy_with_rows(vec![
                TerminalAuthorityPermissionPolicyRow::new(
                    schema,
                    "Only::call\n",
                    TerminalAuthorityDisposition::from_classes([]),
                ),
            ]),
            Err(TerminalAuthorityPermissionPolicyBuildError::InvalidRequirement)
        );
    }

    #[test]
    fn policy_identity_commits_permissions() {
        let schema = schema("Only");
        let empty = terminal_authority_permission_policy_with_rows(vec![
            TerminalAuthorityPermissionPolicyRow::new(
                schema,
                "Only::call",
                TerminalAuthorityDisposition::from_classes([]),
            ),
        ])
        .expect("empty permission");
        let terminating = terminal_authority_permission_policy_with_rows(vec![
            TerminalAuthorityPermissionPolicyRow::new(
                schema,
                "Only::call",
                TerminalAuthorityDisposition::from_classes([
                    TerminalAuthorityClass::ProcessTermination,
                ]),
            ),
        ])
        .expect("terminating permission");
        assert_ne!(empty.identity(), terminating.identity());
    }
}
