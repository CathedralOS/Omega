//! Receiving permission policy for D45's service/terminal containment join.

pub use effects::ServiceTerminalAuthorityPermission as TerminalAuthorityPermissionPolicyRow;
use effects::{
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,
    provider_plan::ServiceSchemaDigest,
};

pub(crate) const TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION: u32 = 1;

/// Exact receiving policy accepted for one native realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPermissionPolicy {
    identity: TerminalAuthorityPermissionPolicyIdentity,
    /// Identity table every row's `requirement_identity` index resolves against.
    identities: terminal_psi::TerminalIdentityTable,
    rows: Vec<TerminalAuthorityPermissionPolicyRow>,
}

impl TerminalAuthorityPermissionPolicy {
    pub const fn identity(&self) -> TerminalAuthorityPermissionPolicyIdentity {
        self.identity
    }

    /// The table the policy rows' `TerminalIdentityIndex` values index.
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
            .cmp(&right.service_schema().as_bytes())
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

/// Empty, exact deny-by-absence policy. Authority-bearing and authority-free
/// terminal leaves both require explicit schema/requirement permission rows.
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
