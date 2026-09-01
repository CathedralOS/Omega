//! Exact package-permission custody shared by direct and retained native routes.

use omega_effects::ServiceTerminalAuthorityPermission;
use psi_diagnostics::Diagnostic;
use std::collections::BTreeSet;

/// Rejoin the rows retained before checked-state destruction to the exact
/// package-acceptance policy supplied independently at retained re-entry.
/// Unlike the receiving policy, this set admits no unrelated extra rows.
pub(super) fn validate_retained_package_terminal_authority_permissions(
    retained: &[ServiceTerminalAuthorityPermission],
    accepted: &omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    if retained == accepted.rows() {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(
            "retained Terminal package permissions differ from the independently accepted package policy",
        )])
    }
}

/// Rejoin every package-approved permission to the independently supplied
/// receiving policy. The receiving policy may contain rows for other
/// artifacts, but it may neither omit nor alter an approved row.
pub(super) fn validate_package_terminal_authority_permissions<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    policy: &omega_terminal_psi_to_native_artifact::TerminalAuthorityPermissionPolicy,
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
