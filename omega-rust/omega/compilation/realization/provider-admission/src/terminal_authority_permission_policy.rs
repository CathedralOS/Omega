/*
Receiving permission policy and package-permission custody for service/terminal
containment.

`TerminalAuthorityPermissionPolicy` holds the exact sorted `(service_schema,
requirement_identity)` permission rows accepted for one native realization. Both
authority-bearing and authority-free terminal leaves require explicit
schema/requirement permission rows under this deny-by-absence policy.

`validate_retained_package_terminal_authority_permissions` rejoins the rows
retained before checked-state destruction to the exact package-acceptance policy
supplied independently at retained re-entry: unlike the receiving policy, it
admits no unrelated extra rows and requires every retained rule and every
accepted rule to match one-for-one.
`validate_package_terminal_authority_permission_custody` rejects repeated
`(service_schema, requirement)` coordinates across the checked compilation's
resolved bindings even when no receiving policy was supplied.
*/

use std::collections::BTreeSet;

use diagnostics::Diagnostic;
pub use effects::ServiceTerminalAuthorityPermission as TerminalAuthorityPermissionPolicyRow;
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityDisposition,
    TerminalAuthorityPermissionPolicyIdentity, provider_plan::ServiceSchemaDigest,
};

pub(crate) const TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPermissionPolicy {
    identity: TerminalAuthorityPermissionPolicyIdentity,
    identities: terminal_psi::TerminalIdentityTable, // Table every row's `requirement_identity` index resolves against.
    rows: Vec<TerminalAuthorityPermissionPolicyRow>,
}

impl TerminalAuthorityPermissionPolicy {
    pub const fn identity(&self) -> TerminalAuthorityPermissionPolicyIdentity {
        self.identity
    }

    pub const fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }

    pub fn rows(&self) -> &[TerminalAuthorityPermissionPolicyRow] {
        &self.rows
    }

    pub(crate) fn permission_for(
        &self,
        service_schema: ServiceSchemaDigest,
        requirement_identity: &str,
    ) -> Result<TerminalAuthorityDisposition, MissingTerminalAuthorityPermission> {
        self.rows
            .iter()
            .find(|row| {
                row.service_schema() == service_schema
                    && self.identities.text(row.requirement_identity())
                        == Some(requirement_identity)
            })
            .map(|row| row.permitted().clone())
            .ok_or_else(|| MissingTerminalAuthorityPermission {
                service_schema,
                requirement_identity: requirement_identity.to_owned(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingTerminalAuthorityPermission {
    service_schema: ServiceSchemaDigest,
    requirement_identity: String,
}

impl MissingTerminalAuthorityPermission {}

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
    identities: terminal_psi::TerminalIdentityTable,
) -> Result<TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError> {
    if rows
        .iter()
        .any(|row| row.requirement_identity().is_absent())
    {
        return Err(TerminalAuthorityPermissionPolicyBuildError::EmptyRequirement);
    }
    if rows.iter().any(|row| {
        identities
            .text(row.requirement_identity())
            .unwrap_or_default()
            .chars()
            .any(char::is_control)
    }) {
        return Err(TerminalAuthorityPermissionPolicyBuildError::InvalidRequirement);
    }
    rows.sort_by(|left, right| {
        left.service_schema()
            .as_bytes()
            .cmp(right.service_schema().as_bytes())
            .then_with(|| {
                identities
                    .text(left.requirement_identity())
                    .cmp(&identities.text(right.requirement_identity()))
            })
    });
    if let Some(rows) = rows.windows(2).find(|rows| {
        rows[0].service_schema() == rows[1].service_schema()
            && rows[0].requirement_identity() == rows[1].requirement_identity()
    }) {
        return Err(
            TerminalAuthorityPermissionPolicyBuildError::DuplicatePermission {
                service_schema: rows[0].service_schema(),
                requirement_identity: identities
                    .text(rows[0].requirement_identity())
                    .unwrap_or_default()
                    .to_owned(),
            },
        );
    }
    let identity = permission_policy_identity(&rows, &identities);
    Ok(TerminalAuthorityPermissionPolicy {
        identity,
        identities,
        rows,
    })
}

fn permission_policy_identity(
    rows: &[TerminalAuthorityPermissionPolicyRow],
    identities: &terminal_psi::TerminalIdentityTable,
) -> TerminalAuthorityPermissionPolicyIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.terminal-authority.permission-policy.v1\0",
    );
    digest.raw(&TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION.to_be_bytes());
    digest.raw(&(rows.len() as u64).to_be_bytes());
    for row in rows {
        digest.raw(row.service_schema().as_bytes());
        let requirement = identities
            .text(row.requirement_identity())
            .unwrap_or_default();
        digest.raw(&(requirement.len() as u64).to_be_bytes());
        digest.raw(requirement.as_bytes());
        digest.raw(&(row.permitted().classes().len() as u64).to_be_bytes());
        for class in row.permitted().classes() {
            digest.byte(class.canonical_tag());
        }
    }
    TerminalAuthorityPermissionPolicyIdentity::from_parts(
        TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
        digest.finish(),
    )
}

pub fn validate_retained_package_terminal_authority_permissions(
    retained: &[ServiceTerminalAuthorityPermission],
    retained_identities: &terminal_psi::TerminalIdentityTable,
    accepted: &crate::provider_admission::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in retained {
        let requirement = retained_identities
            .text(permission.requirement_identity())
            .unwrap_or_default()
            .to_owned();
        let coordinate = (permission.service_schema(), requirement.clone());
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission custody repeats terminal-authority permission `{requirement}` for one exact service schema",
            )));
            continue;
        }
        match accepted.rows().iter().find(|row| {
            row.service_schema() == permission.service_schema()
                && accepted.identities().text(row.requirement_identity())
                    == Some(requirement.as_str())
        }) {
            Some(row) if row.permitted() == permission.permitted() => {}
            Some(_) => diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission for `{requirement}` alters the accepted disposition",
            ))),
            None => diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission for `{requirement}` has no accepted package row",
            ))),
        }
    }
    for row in accepted.rows() {
        let requirement = accepted
            .identities()
            .text(row.requirement_identity())
            .unwrap_or_default();
        if !seen.contains(&(row.service_schema(), requirement.to_owned())) {
            diagnostics.push(Diagnostic::error(format!(
                "accepted package terminal-authority permission for `{requirement}` was not retained",
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        let mut all = vec![Diagnostic::error(
            "retained Terminal package permissions differ from the independently accepted package policy",
        )];
        all.append(&mut diagnostics);
        Err(all)
    }
}

pub fn validate_package_terminal_authority_permission_custody<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in permissions {
        let requirement = identities
            .text(permission.requirement_identity())
            .unwrap_or_default()
            .to_owned();
        let coordinate = (permission.service_schema(), requirement.clone());
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "package permission custody repeats terminal-authority permission `{requirement}` for one exact service schema",
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
