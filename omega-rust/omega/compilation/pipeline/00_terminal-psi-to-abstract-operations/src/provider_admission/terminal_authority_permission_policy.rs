//! Receiving permission policy for D45's service/terminal containment join.

pub use effects::ServiceTerminalAuthorityPermission as TerminalAuthorityPermissionPolicyRow;
use effects::{
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,

pub(crate) const TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION: u32 = 1;

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

    pub(crate) fn permission_for(
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
fn permission_policy_identity(
    rows: &[TerminalAuthorityPermissionPolicyRow],
) -> TerminalAuthorityPermissionPolicyIdentity {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.terminal-authority.permission-policy.v1\0",
    );
    digest.raw(&TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION.to_be_bytes());
    digest.raw(&(rows.len() as u64).to_be_bytes());
    for row in rows {
        digest.raw(row.service_schema().as_bytes());
        digest.raw(&(row.requirement_identity().len() as u64).to_be_bytes());
        digest.raw(row.requirement_identity().as_bytes());
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
