//! Exact package-permission custody shared by direct and retained native routes.

use diagnostics::Diagnostic;
use effects::ServiceTerminalAuthorityPermission;
use std::collections::BTreeSet;

/// Rejoin the rows retained before checked-state destruction to the exact
/// package-acceptance policy supplied independently at retained re-entry.
/// Unlike the receiving policy, this set admits no unrelated extra rows.
pub fn validate_retained_package_terminal_authority_permissions(
    retained: &[ServiceTerminalAuthorityPermission],
    accepted: &crate::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    if retained == accepted.rows() {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(
            "retained Terminal package permissions differ from the independently accepted package policy",
        )])
    }
}

/// Package permission custody independent of any receiving policy: reject a
/// repeated `(service schema, requirement)` coordinate across the checked
/// compilation's resolved bindings. This runs even when no receiving policy
/// was supplied; absence of a receiving policy is not a substitute for the
/// package axis.
pub fn validate_package_terminal_authority_permission_custody<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
) -> Result<(), Vec<Diagnostic>> {
    validate_permission_custody(permissions, None)
}

/// Rejoin every package-approved permission to the independently supplied
/// receiving policy. The receiving policy may contain rows for other
/// artifacts, but it may neither omit nor alter an approved row.
pub fn validate_package_terminal_authority_permissions<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    policy: &crate::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    validate_permission_custody(permissions, Some(policy))
}

fn validate_permission_custody<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    policy: Option<&crate::TerminalAuthorityPermissionPolicy>,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in permissions {
        let coordinate = (
            permission.service_schema(),
            permission.requirement_identity().to_owned(),
        );
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "package permission custody repeats terminal-authority permission `{}` for one exact service schema",
                permission.requirement_identity(),
            )));
            continue;
        }
        let Some(policy) = policy else {
            continue;
        };
        match policy.permission_for(
            permission.service_schema(),
            permission.requirement_identity(),
        ) {
            Ok(permitted) if &permitted == permission.permitted() => {}
            Ok(_) => diagnostics.push(Diagnostic::error(format!(
                "receiving terminal-authority policy substitutes the accepted permission for `{}`",
                permission.requirement_identity(),
            ))),
            Err(_) => diagnostics.push(Diagnostic::error(format!(
                "receiving terminal-authority policy omits the accepted permission for `{}`",
                permission.requirement_identity(),
            ))),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
